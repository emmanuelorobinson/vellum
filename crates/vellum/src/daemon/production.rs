//! The daemon's real parts: the file system, the codec, the Loro store, the scene and the Skia renderer, put
//! together the way ARCHITECTURE §10 composes a native host. Everything here is a choice of adapter; what the
//! daemon does with them is in the rest of `daemon` and in `tools`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{BuildHasher, Hasher};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use vellum_assets_fs::{StdFs, content_hash};
use vellum_codec_json::JsonCodec;
use vellum_core::doc_api::{NoServices, Services};
use vellum_core::ports::{Clock, DocStore, FileSystem, FileWatcher, FsError, IdGen, Timestamp};
use vellum_doc_loro::LoroDocStore;
use vellum_types::NodeId;
use vellum_watch_notify::NotifyWatcher;

use super::{Documents, Env, Recovery};
use crate::tools::Registry;

/// The wall clock. It never goes backwards, which `Clock` promises and a system clock does not.
pub struct SystemClock(AtomicI64);

impl SystemClock {
    /// A clock reading the system time.
    pub fn new() -> Self {
        Self(AtomicI64::new(0))
    }
}

impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for SystemClock {
    fn now(&self) -> Timestamp {
        let ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX));
        Timestamp(self.0.fetch_max(ms, Ordering::Relaxed).max(ms))
    }
}

/// Ten random characters from the node id alphabet (guide §9). The operating system is the source of
/// randomness, so two daemons never agree on an id by chance.
pub struct RandomIds;

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_-";

impl IdGen for RandomIds {
    fn node_id(&mut self) -> NodeId {
        let mut bytes = [0u8; 10];
        // An operating system that cannot give randomness leaves nothing safe to continue with.
        getrandom::fill(&mut bytes).expect("the operating system provides randomness");
        let id: String = bytes
            .iter()
            .map(|b| ALPHABET[usize::from(b & 63)] as char)
            .collect();
        NodeId::new(id).expect("the alphabet is the id alphabet")
    }
}

/// 80 bits from the operating system, for the random part of agent and session ids.
fn entropy() -> u128 {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).expect("the operating system provides randomness");
    u128::from_le_bytes(bytes)
}

/// The sandbox `execute` scripts run in: QuickJS with a monotonic clock for its time limit. A script never sees
/// the clock, and its random numbers come from a fixed seed, so a run does not depend on when it happened.
pub fn script_runtime() -> Option<Rc<dyn vellum_core::ports::ScriptRuntime>> {
    #[cfg(feature = "scripts")]
    {
        let start = std::time::Instant::now();
        Some(Rc::new(vellum_script_quickjs::QuickJsRuntime::new(
            move || u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX),
            0x5EED,
        )))
    }
    #[cfg(not(feature = "scripts"))]
    None
}

/// A random Loro peer id. The store wants one below 2^63; the top bit names a document's genesis.
fn peer() -> u64 {
    std::collections::hash_map::RandomState::new()
        .build_hasher()
        .finish()
        >> 1
}

/// Layout, text, the spatial index and Skia, as the daemon wires them. Without a renderer (Skia cannot start
/// on this machine, or the `scene` feature is off) the services say so on every call instead of the daemon
/// failing to start.
pub fn scene_services() -> Rc<dyn Services> {
    #[cfg(feature = "scene")]
    if let Some(services) = scene::services() {
        return services;
    }
    Rc::new(NoServices)
}

#[cfg(feature = "scene")]
mod scene {
    use std::cell::RefCell;
    use std::rc::Rc;

    use vellum_core::display_list;
    use vellum_core::doc_api::{RenderRead, Rendered, SceneConfig, SceneServices, Services, error};
    use vellum_core::ports::{Rect, ScriptError};
    use vellum_core::resolver::libraries::LibrarySource;
    use vellum_types::Document;

    struct NoLibraries;

    impl LibrarySource for NoLibraries {
        fn load(&self, _: &str) -> Result<Option<Document>, String> {
            // Libraries are read through the file system with the document's own path, which the daemon's
            // single set of scene services does not have (reports.md, Prompt 29).
            Ok(None)
        }
    }

    struct Skia(vellum_render_skia::Renderer);

    impl RenderRead for Skia {
        fn render(
            &self,
            output: &display_list::Output,
            bounds: Rect,
            scale: f64,
            format: &str,
        ) -> Result<Rendered, ScriptError> {
            let resources = vellum_render_skia::Resources {
                fonts: output.fonts.clone(),
                ..Default::default()
            };
            let scene = self.0.prepare(&output.list, &resources).map_err(error)?;
            let page = vellum_render_skia::Page {
                width: (bounds.width * scale).ceil() as u32,
                height: (bounds.height * scale).ceil() as u32,
                scene: &scene,
            };
            let (mime, bytes) = match format {
                "png" => ("image/png", self.0.png(&page).map_err(error)?),
                "svg" => (
                    "image/svg+xml",
                    self.0.svg(&page).map_err(error)?.into_bytes(),
                ),
                "pdf" => ("application/pdf", self.0.pdf(&[page]).map_err(error)?),
                _ => return Err(error("Unsupported format")),
            };
            Ok(Rendered {
                mime: mime.into(),
                bytes,
            })
        }
    }

    pub(super) fn services() -> Option<Rc<dyn Services>> {
        let renderer = vellum_render_skia::Renderer::new().ok()?;
        let fonts = vellum_text_parley::ParleyFonts::new();
        let text = vellum_text_parley::ParleyText::new(fonts.clone());
        Some(Rc::new(SceneServices::new(
            SceneConfig {
                script_outputs: Default::default(),
                path: String::new(),
                libraries: Rc::new(NoLibraries),
                layout: RefCell::new(Box::new(vellum_layout_taffy::TaffyLayout::new())),
                text: Rc::new(text),
                fonts: Rc::new(fonts),
                renderer: Rc::new(Skia(renderer)),
            },
            vellum_index_rstar::RstarIndex::new,
        )))
    }
}

/// The nearest folder above `path` that holds a `.git`, which is where a repo's documents are rooted so that
/// `../` into a library or an assets folder stays inside it.
fn repo_root(path: &Path) -> Option<PathBuf> {
    path.ancestors()
        .skip(1)
        .find(|dir| dir.join(".git").symlink_metadata().is_ok())
        .map(Path::to_path_buf)
}

/// Where a document path lives on the real disk. A document path is absolute here (the tools make it so from the
/// client's working directory) and names a `.vellum` file or folder; nothing else is reachable through it.
pub fn locate() -> super::documents::Locate {
    let roots: RefCell<HashMap<PathBuf, Arc<StdFs>>> = RefCell::new(HashMap::new());
    Box::new(move |path: &str| {
        let full = Path::new(path);
        if !full.is_absolute() {
            return Err(FsError::InvalidPath(format!(
                "{path} is not an absolute path. Open the document from a client that reports its working directory, or give the full path."
            )));
        }
        if full.extension().is_none_or(|ext| ext != "vellum") {
            return Err(FsError::InvalidPath(format!(
                "{path} is not a document: documents end in .vellum."
            )));
        }
        let root = repo_root(full)
            .or_else(|| full.parent().map(Path::to_path_buf))
            .ok_or_else(|| FsError::InvalidPath(path.to_owned()))?;
        let inner = full
            .strip_prefix(&root)
            .map_err(|_| FsError::InvalidPath(path.to_owned()))?
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/");
        let fs = roots
            .borrow_mut()
            .entry(root.clone())
            .or_insert_with(|| Arc::new(StdFs::new(root)))
            .clone();
        Ok((fs as Arc<dyn FileSystem>, inner))
    })
}

struct NoWatcher;

impl FileWatcher for NoWatcher {
    fn id(&self) -> &'static str {
        "none"
    }
    fn watch(&mut self, _: &str) -> Result<(), FsError> {
        Ok(())
    }
    fn unwatch(&mut self, _: &str) {}
    fn changed(&mut self) -> Vec<String> {
        Vec::new()
    }
}

/// The documents of a daemon whose private folder is `state` (`~/.vellum`, or its stand-in under test).
pub fn documents(state: &Path) -> std::io::Result<Documents> {
    let recovery = super::recovery::recovery_dir_in(state)?;
    let codec = Arc::new(JsonCodec::new());
    let env = Env {
        locate: locate(),
        codec: codec.clone(),
        hash: content_hash,
        clock: Arc::new(SystemClock::new()),
        store: Box::new(|doc| Box::new(LoroDocStore::new(doc, peer())) as Box<dyn DocStore>),
        ids: Box::new(|| Box::new(RandomIds)),
        services: scene_services(),
        scripts: script_runtime(),
        entropy: Box::new(entropy),
    };
    // A machine that cannot watch files still serves tools; an edit made outside is seen at the next open.
    let watcher: Box<dyn FileWatcher> = match NotifyWatcher::new(Duration::from_millis(150)) {
        Ok(watcher) => Box::new(watcher),
        Err(_) => Box::new(NoWatcher),
    };
    Ok(Documents::new(
        env,
        watcher,
        Recovery::new(Arc::new(StdFs::new(recovery)), codec, content_hash),
    ))
}

/// What `vellum daemon` answers MCP with: the standard registry over the real documents. It is built on the
/// thread that runs it, because the documents hold state that cannot move between threads.
pub fn handler(state: PathBuf) -> vellum_ipc_local::HandlerFactory {
    Box::new(move || match documents(&state) {
        Ok(docs) => Box::new(
            crate::mcp::McpServer::new(Registry::standard(), docs)
                .with_default_cwd(std::env::current_dir().ok()),
        ),
        Err(error) => Box::new(Unavailable(error.to_string())),
    })
}

/// A daemon whose documents could not be set up still answers, with the reason, instead of hanging up.
struct Unavailable(String);

impl vellum_ipc_local::RpcHandler for Unavailable {
    fn request(&mut self, _: u64, request: serde_json::Value) -> Option<serde_json::Value> {
        let id = request.get("id")?.clone();
        Some(serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {"code": -32603, "message": format!("The daemon could not set up its documents: {}", self.0)},
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_peer_id_is_always_below_two_to_the_63() {
        // Loro refuses higher ones (they name a document's genesis); one bad draw would panic a tool call.
        for _ in 0..10_000 {
            assert!(peer() < 1 << 63);
        }
    }

    #[test]
    fn random_ids_are_ten_characters_of_the_id_alphabet_and_do_not_repeat() {
        let mut ids = RandomIds;
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..1_000 {
            let id = ids.node_id();
            assert_eq!(id.as_str().len(), 10);
            assert!(seen.insert(id.as_str().to_owned()), "{id} repeated");
        }
    }

    #[test]
    fn the_clock_never_goes_backwards() {
        let clock = SystemClock::new();
        let mut last = clock.now();
        for _ in 0..1_000 {
            let now = clock.now();
            assert!(now >= last);
            last = now;
        }
        assert!(last.0 > 1_700_000_000_000, "it reads the real time");
    }

    #[test]
    fn documents_are_absolute_vellum_paths_rooted_at_their_repo() {
        let root = std::env::temp_dir().join(format!("v-loc-{}", std::process::id()));
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::create_dir_all(root.join("design")).unwrap();
        let locate = locate();
        let file = root.join("design/app.vellum");
        let (_, inner) = locate(file.to_str().unwrap()).expect("a document in a repo");
        assert_eq!(
            inner, "design/app.vellum",
            "rooted at the repo, so ../libs stays inside it"
        );
        for refused in [
            "design/app.vellum",
            root.join("design/notes.txt").to_str().unwrap(),
            root.join("design").to_str().unwrap(),
        ] {
            assert!(locate(refused).is_err(), "{refused}");
        }
        let _ = std::fs::remove_dir_all(&root);
    }
}
