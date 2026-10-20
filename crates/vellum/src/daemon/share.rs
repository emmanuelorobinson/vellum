//! Explicit portable snapshots. Dependency discovery reads only model references, never a repo glob.
mod dependencies;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use vellum_core::ports::{FileSystem, FsError};
use vellum_types::Document;
const LIMIT: usize = 64 * 1024 * 1024;
#[derive(Clone, Serialize, Deserialize)]
pub struct File {
    pub path: String,
    pub data: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Frame {
    pub id: String,
    pub name: String,
    pub width: f64,
    pub height: f64,
    pub display_list: String,
    pub fonts: Vec<Font>,
    pub images: Vec<Image>,
    pub spec: serde_json::Value,
    pub css: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Font {
    pub handle: String,
    pub index: u32,
    pub path: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Image {
    pub id: String,
    pub path: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Bundle {
    pub schema: u32,
    pub entry: String,
    pub files: Vec<File>,
    pub missing: Vec<String>,
    pub frames: Vec<Frame>,
}
pub fn base64(bytes: &[u8]) -> String {
    const ABC: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in bytes.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        for i in 0..4 {
            out.push(if i > c.len() {
                '='
            } else {
                ABC[((n >> (18 - 6 * i)) & 63) as usize] as char
            });
        }
    }
    out
}
fn unbase64(s: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let (mut n, mut bits) = (0u32, 0);
    for b in s.bytes() {
        if b == b'=' {
            break;
        }
        let v = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return Err("Invalid snapshot bytes".into()),
        };
        n = (n << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((n >> bits) as u8);
        }
    }
    Ok(out)
}
fn safe(p: &str) -> Result<(), String> {
    vellum_core::ports::check_path(p, false).map_err(|_| "Dependency escapes the repo")?;
    if p.len() > 1024
        || p.split('/').any(|p| p.starts_with('.'))
        || p.chars().any(|c| c.is_control() || c == ':')
    {
        return Err("Unsafe snapshot path".into());
    }
    Ok(())
}
fn relative(owner: &str, path: &str) -> Result<String, String> {
    if path.starts_with('/') || path.contains(':') || path.contains('\\') {
        return Err("Dependency escapes the repo".into());
    }
    let dir = owner.rsplit_once('/').map_or("", |x| x.0);
    let mut parts: Vec<&str> = dir.split('/').filter(|p| !p.is_empty()).collect();
    for part in path.split('/') {
        match part {
            ".." => {
                if parts.pop().is_none() {
                    return Err("Dependency escapes the repo".into());
                }
            }
            "." => {}
            "" => return Err("Unsafe snapshot path".into()),
            p => parts.push(p),
        }
    }
    let p = parts.join("/");
    safe(&p)?;
    Ok(p)
}
impl Bundle {
    pub fn decoded_files(&self) -> Result<BTreeMap<String, Vec<u8>>, String> {
        self.files
            .iter()
            .map(|f| Ok((f.path.clone(), unbase64(&f.data)?)))
            .collect()
    }
    pub fn bytes(&self) -> Result<Vec<u8>, String> {
        let b = serde_json::to_vec(self).map_err(|_| "Snapshot unavailable")?;
        if b.len() > LIMIT {
            return Err("Snapshot exceeds 64 MiB".into());
        }
        Ok(b)
    }
}
pub fn collect(fs: Arc<dyn FileSystem>, entry: &str, doc: &Document) -> Result<Bundle, String> {
    safe(entry)?;
    let codec = vellum_codec_json::JsonCodec::new();
    let mut files = BTreeMap::new();
    let mut missing = BTreeSet::new();
    let mut seen = BTreeSet::new();
    let mut docs = vec![(entry.to_owned(), doc.clone())];
    let mut size = 0;
    while let Some((path, doc)) = docs.pop() {
        if !seen.insert(path.clone()) {
            continue;
        }
        if seen.len() > 256 {
            return Err("Too many imported libraries".into());
        }
        if !vellum_core::ops::check_document(&doc).is_empty() {
            return Err("Invalid snapshot document".into());
        }
        files.insert(
            path.clone(),
            codec
                .encode(&doc)
                .map_err(|_| "Document cannot be encoded")?,
        );
        for import in doc.imports.values() {
            let dep = relative(&path, import)?;
            if seen.contains(&dep) {
                continue;
            }
            match (super::DocFile {
                fs: fs.clone(),
                path: dep.clone(),
                codec: Arc::new(vellum_codec_json::JsonCodec::new()),
            })
            .read()
            {
                Ok(loaded) if !loaded.interrupted && loaded.orphans.is_empty() => {
                    docs.push((dep, loaded.document))
                }
                Err(vellum_core::ports::FileError::Fs(FsError::NotFound(_))) => {
                    missing.insert(dep);
                }
                _ => return Err("Library is unreadable or has an incomplete save".into()),
            }
        }
        let refs = dependencies::collect(&doc, &path)?;
        for dep in refs {
            safe(&dep)?;
            if files.contains_key(&dep) {
                continue;
            }
            match fs.read(&dep) {
                Ok(bytes) => {
                    if dep.starts_with(&format!("{}/", vellum_assets_fs::assets_dir_for(&path)))
                        && vellum_assets_fs::content_hash(&bytes)
                            != dep.rsplit('/').next().unwrap_or("")
                    {
                        return Err("Snapshot asset is damaged".into());
                    }
                    size += bytes.len();
                    if size > LIMIT || files.len() >= 2048 {
                        return Err("Snapshot exceeds dependency limits".into());
                    }
                    files.insert(dep, bytes);
                }
                Err(FsError::NotFound(_)) => {
                    missing.insert(dep);
                }
                Err(_) => return Err("Snapshot dependency is unreadable".into()),
            }
        }
    }
    let bundle = Bundle {
        schema: 1,
        entry: entry.into(),
        files: files
            .into_iter()
            .map(|(path, bytes)| File {
                path,
                data: base64(&bytes),
            })
            .collect(),
        missing: missing.into_iter().collect(),
        frames: vec![],
    };
    bundle.bytes()?;
    Ok(bundle)
}
/// Freeze all rendering inputs to the exact uploaded snapshot; renderer code cannot read the checkout.
pub struct Frozen(pub BTreeMap<String, Vec<u8>>);
impl FileSystem for Frozen {
    fn read(&self, p: &str) -> Result<Vec<u8>, FsError> {
        self.0
            .get(p)
            .cloned()
            .ok_or_else(|| FsError::NotFound(p.into()))
    }
    fn exists(&self, p: &str) -> Result<bool, FsError> {
        Ok(self.0.contains_key(p))
    }
    fn list(&self, _: &str) -> Result<Vec<String>, FsError> {
        Err(FsError::Unavailable(
            "Snapshot is a single-file document".into(),
        ))
    }
    fn write(&self, _: &str, _: &[u8]) -> Result<(), FsError> {
        Err(readonly())
    }
    fn sync_file(&self, _: &str) -> Result<(), FsError> {
        Err(readonly())
    }
    fn rename(&self, _: &str, _: &str) -> Result<(), FsError> {
        Err(readonly())
    }
    fn remove_file(&self, _: &str) -> Result<(), FsError> {
        Err(readonly())
    }
    fn create_dir_all(&self, _: &str) -> Result<(), FsError> {
        Err(readonly())
    }
    fn sync_dir(&self, _: &str) -> Result<(), FsError> {
        Err(readonly())
    }
}
fn readonly() -> FsError {
    FsError::Unavailable("Snapshot is read-only".into())
}
fn script_outputs(
    bundle: &Bundle,
) -> Result<BTreeMap<(String, vellum_types::NodeId), vellum_core::script_nodes::Output>, String> {
    use vellum_core::ports::IdGen;
    struct Ids(u64);
    impl IdGen for Ids {
        fn node_id(&mut self) -> vellum_types::NodeId {
            self.0 += 1;
            vellum_types::NodeId::new(format!("snapshot_{}", self.0)).expect("generated id")
        }
    }
    fn nodes(nodes: &[vellum_types::Node], out: &mut Vec<vellum_types::Node>) {
        for node in nodes {
            if matches!(node, vellum_types::Node::Script(_)) {
                out.push(node.clone());
            }
            nodes_walk(node, out);
        }
    }
    fn nodes_walk(node: &vellum_types::Node, out: &mut Vec<vellum_types::Node>) {
        nodes(node.children(), out);
        if let vellum_types::Node::Instance(instance) = node {
            for content in instance.slot_content.values() {
                nodes(content, out);
            }
        }
    }
    let files = bundle.decoded_files()?;
    let mut outputs = BTreeMap::new();
    let mut ids = Ids(0);
    let mut runtime = None;
    for (path, bytes) in &files {
        if !path.ends_with(".vellum") {
            continue;
        }
        let doc = vellum_codec_json::JsonCodec::new()
            .decode(bytes)
            .map_err(|_| "Invalid snapshot document")?;
        let mut scripts = vec![];
        for page in &doc.pages {
            nodes(&page.children, &mut scripts);
        }
        for component in doc.components.values() {
            nodes(std::slice::from_ref(&component.root), &mut scripts);
        }
        for node in scripts {
            if outputs.len() >= 32 {
                return Err("Snapshot exceeds 32 script nodes".into());
            }
            let vellum_types::Node::Script(script) = &node else {
                unreachable!()
            };
            let source = relative(path, &script.src)?;
            let source =
                std::str::from_utf8(files.get(&source).ok_or("Snapshot script is missing")?)
                    .map_err(|_| "Invalid script encoding")?;
            if runtime.is_none() {
                runtime = super::production::script_runtime();
            }
            let runtime = runtime
                .as_ref()
                .ok_or("Snapshot script runtime unavailable")?;
            let output = vellum_core::script_nodes::evaluate(
                &**runtime,
                &doc,
                &node,
                source,
                &Default::default(),
                &mut ids,
            )
            .map_err(|_| "Snapshot script could not be evaluated")?;
            outputs.insert((path.clone(), node.id().clone()), output);
        }
    }
    Ok(outputs)
}

pub fn render(bundle: &mut Bundle, doc: &Document) -> Result<(), String> {
    let fs: Arc<dyn FileSystem> = Arc::new(Frozen(bundle.decoded_files()?));
    let outputs = script_outputs(bundle)?;
    let services = super::production::snapshot_services(doc, fs.clone(), &bundle.entry, outputs)
        .map_err(|_| "Snapshot renderer unavailable")?;
    let targets: Vec<String> = doc
        .pages
        .iter()
        .flat_map(|p| {
            p.children
                .iter()
                .filter(|n| !matches!(n.visible(), vellum_types::Value::Literal(false)))
                .map(|n| n.id().as_str().to_owned())
        })
        .collect();
    if targets.is_empty() {
        return Ok(());
    }
    if targets.len() > 100 {
        return Err("Snapshot exceeds 100 frames".into());
    }
    let scene = crate::export_scene::prepare(doc, fs, &bundle.entry, &*services, &targets)
        .map_err(|_| "Snapshot render is incomplete")?;
    for (i, f) in scene.frames.into_iter().enumerate() {
        fn complete(node: &vellum_core::resolver::ResolvedNode) -> bool {
            node.problems.is_empty()
                && !matches!(node.value.node, vellum_types::Node::Script(_))
                && node.children.iter().all(|child| complete(child))
        }
        if !f.roots.iter().all(|root| complete(root)) {
            return Err("Snapshot has unresolved nodes or script inputs".into());
        }
        if !f.output.problems.is_empty() {
            return Err("Snapshot render is incomplete".into());
        }
        use vellum_core::ports::{CodeExporter, ExportRenderer};
        struct Render<'a>(&'a dyn vellum_core::doc_api::Services);
        impl ExportRenderer for Render<'_> {
            fn render(
                &self,
                frames: &[vellum_core::export::ExportFrame],
                format: &str,
            ) -> Result<Vec<u8>, vellum_core::ports::ExportError> {
                self.0.render_export(frames, format)
            }
        }
        let html = vellum_export_html::Html::new("html-css")
            .expect("format")
            .export(
                &vellum_core::export::ExportScene {
                    frames: vec![f.clone()],
                },
                &vellum_core::export::CodeOptions {
                    scaffold: false,
                    assets: vellum_core::export::AssetMode::Inline,
                },
                &Render(&*services),
            )
            .map_err(|_| "Snapshot code export unavailable")?;
        let css = html
            .files
            .first()
            .and_then(|file| std::str::from_utf8(&file.bytes).ok())
            .and_then(|html| html.strip_prefix("<style>"))
            .and_then(|html| html.split_once("</style>"))
            .map_or(String::new(), |x| x.0.to_owned());
        let display_list = format!("viewer/frame-{i}.bin");
        bundle.files.push(File {
            path: display_list.clone(),
            data: base64(
                &f.output
                    .list
                    .to_bytes()
                    .map_err(|_| "Invalid display list")?,
            ),
        });
        let mut fonts = vec![];
        for (h, data) in f.output.fonts {
            let path = format!(
                "viewer/font-{}",
                vellum_assets_fs::content_hash(&data.bytes)
            );
            if !bundle.files.iter().any(|f| f.path == path) {
                bundle.files.push(File {
                    path: path.clone(),
                    data: base64(&data.bytes),
                });
            }
            fonts.push(Font {
                handle: h.0.to_string(),
                index: data.index,
                path,
            });
        }
        let mut images = vec![];
        for (id, data) in f.images {
            let path = format!("viewer/image-{}", id.as_str());
            if !bundle.files.iter().any(|f| f.path == path) {
                bundle.files.push(File {
                    path: path.clone(),
                    data: base64(&data),
                });
            }
            images.push(Image {
                id: id.as_str().into(),
                path,
            });
        }
        let spec = {
            use vellum_core::ports::SpecFormat;
            vellum_types::NodeId::new(f.id.clone())
                .ok()
                .and_then(|id| vellum_core::spec::LayoutSpecV1.build(doc, &id).ok())
                .unwrap_or(serde_json::Value::Null)
        };
        bundle.frames.push(Frame {
            css,
            spec,
            id: f.id,
            name: f.name,
            width: f.bounds.width,
            height: f.bounds.height,
            display_list,
            fonts,
            images,
        });
    }
    bundle.bytes()?;
    Ok(())
}

use serde_json::{Value, json};
use std::{path::PathBuf, sync::Mutex};
use vellum_account_http::{AccountClient, StoredSession};
use vellum_core::ports::SecretStore;
/// A job outlives its requesting connection. Failed/restarted jobs resume the same immutable bytes.
pub struct Uploads {
    root: PathBuf,
    jobs: BTreeMap<String, Arc<Mutex<Value>>>,
}
impl Uploads {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            jobs: BTreeMap::new(),
        }
    }
    fn path(&self, document: &str) -> PathBuf {
        self.root.join(format!(
            "{}.json",
            vellum_assets_fs::content_hash(document.as_bytes())
        ))
    }
    pub fn status(&mut self, document: &str) -> Value {
        if let Some(state) = self.jobs.get(document) {
            return state.lock().unwrap().clone();
        }
        let path = self.path(document);
        if let Ok(bytes) = std::fs::read(path)
            && let Ok(v) = serde_json::from_slice::<Value>(&bytes)
        {
            if v["public"]["state"] == "shared" {
                return v["public"].clone();
            }
            return json!({"state":"failed","id":v["id"],"sha256":v["sha256"],"message":"Upload paused; retry to resume"});
        }
        json!({"state":"not-shared"})
    }
    pub fn start(
        &mut self,
        document: &str,
        bundle: Option<Bundle>,
        share_id: Option<String>,
        source_hash: Option<String>,
    ) -> Result<Value, String> {
        if self
            .jobs
            .get(document)
            .is_some_and(|j| j.lock().unwrap()["state"] == "uploading")
        {
            return Err("An upload is already running".into());
        }
        if self
            .jobs
            .values()
            .filter(|j| j.lock().unwrap()["state"] == "uploading")
            .count()
            >= 4
        {
            return Err("Four uploads are already running".into());
        }
        std::fs::create_dir_all(&self.root).map_err(|_| "Upload cache unavailable")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&self.root, std::fs::Permissions::from_mode(0o700))
                .map_err(|_| "Upload cache unavailable")?;
        }
        let path = self.path(document);
        let mut record = if let Some(bundle) = bundle {
            json!({"bundle":bundle,"id":share_id,"sourceHash":source_hash})
        } else {
            serde_json::from_slice(&std::fs::read(&path).map_err(|_| "No upload to retry")?)
                .map_err(|_| "Upload cache invalid")?
        };
        let bundle: Bundle =
            serde_json::from_value(record["bundle"].clone()).map_err(|_| "Upload cache invalid")?;
        let bytes = bundle.bytes()?;
        record["sha256"] = json!(vellum_assets_fs::content_hash(&bytes));
        persist(&path, &record)?;
        let state = Arc::new(Mutex::new(
            json!({"state":"uploading","progress":0,"missing":bundle.missing}),
        ));
        self.jobs.insert(document.into(), state.clone());
        std::thread::spawn(move || {
            let result = upload(&path, &mut record, &state);
            if let Err(message) = result {
                let mut failed = json!({"state":"failed","message":message});
                if let Some(id) = record["id"].as_str() {
                    failed["id"] = json!(id);
                }
                *state.lock().unwrap() = failed;
            }
        });
        Ok(self.status(document))
    }
    pub fn revoke(&mut self, document: &str, id: &str) -> Result<Value, String> {
        // Cloud revoke races safely with publishing via the service's conditional record write.
        let session = cloud_session()?;
        let client = AccountClient::new(session.origin).map_err(str::to_owned)?;
        client
            .share("DELETE", &format!("v1/shares/{id}"), &[], &session.token)
            .map_err(str::to_owned)?;
        if let Some(state) = self.jobs.get(document) {
            *state.lock().unwrap() = json!({"state":"not-shared"});
        }
        let _ = std::fs::remove_file(self.path(document));
        Ok(json!({"state":"not-shared"}))
    }
}
fn persist(path: &std::path::Path, value: &Value) -> Result<(), String> {
    use std::io::Write;
    let tmp = path.with_extension("tmp");
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&tmp).map_err(|_| "Upload cache unavailable")?;
    let result = (|| {
        file.write_all(&serde_json::to_vec(value).map_err(|_| "Upload cache invalid")?)
            .map_err(|_| "Upload cache unavailable")?;
        file.sync_all().map_err(|_| "Upload cache unavailable")?;
        std::fs::rename(&tmp, path).map_err(|_| "Upload cache unavailable")
    })();
    let _ = std::fs::remove_file(tmp);
    result.map_err(str::to_owned)
}
fn cloud_session() -> Result<StoredSession, String> {
    vellum_secrets_keyring::Keychain::new(vellum_types::brand::KEYCHAIN_SERVICE)
        .get("cloud-session")
        .map_err(|_| "OS keychain unavailable")?
        .ok_or("Sign in required")
        .and_then(StoredSession::decode)
        .map_err(str::to_owned)
}
fn upload(path: &std::path::Path, record: &mut Value, state: &Mutex<Value>) -> Result<(), String> {
    let mut bundle: Bundle =
        serde_json::from_value(record["bundle"].clone()).map_err(|_| "Upload cache invalid")?;
    if bundle.missing.is_empty() && bundle.frames.is_empty() {
        let files = bundle.decoded_files()?;
        let doc = vellum_codec_json::JsonCodec::new()
            .decode(
                files
                    .get(&bundle.entry)
                    .ok_or("Snapshot entry unavailable")?,
            )
            .map_err(|_| "Invalid snapshot document")?;
        render(&mut bundle, &doc)?;
    }
    let bytes = bundle.bytes()?;
    record["bundle"] = serde_json::to_value(bundle).map_err(|_| "Upload cache invalid")?;
    record["sha256"] = json!(vellum_assets_fs::content_hash(&bytes));
    persist(path, record)?;
    let session = cloud_session()?;
    let origin = session.origin.as_str().to_owned();
    let client = AccountClient::new(session.origin).map_err(str::to_owned)?;
    let request = |method: &str, path: &str, body: &[u8]| {
        client
            .share(method, path, body, &session.token)
            .map_err(str::to_owned)
    };
    drive_upload(path, record, &bytes, state, &origin, request)
}
fn drive_upload(
    path: &std::path::Path,
    record: &mut Value,
    bytes: &[u8],
    state: &Mutex<Value>,
    origin: &str,
    request: impl Fn(&str, &str, &[u8]) -> Result<Value, String>,
) -> Result<(), String> {
    let mut received = vec![];
    record["uploadId"] = Value::Null;
    if let Some(id) = record["id"].as_str()
        && let Ok(s) = request("GET", &format!("v1/shares/{id}/status"), &[])
    {
        if s["snapshot"]["sha256"] == record["sha256"] {
            let url = format!("{origin}share/{id}");
            *state.lock().unwrap() = json!({"state":"shared","id":id,"url":url,"snapshot":s["snapshot"],"sha256":record["sha256"],"sourceHash":record["sourceHash"]});
            record["public"] = state.lock().unwrap().clone();
            persist(path, record)?;
            return Ok(());
        }
        if s["upload"]["sha256"] == record["sha256"] {
            record["uploadId"] = s["upload"]["id"].clone();
            received = s["received"].as_array().cloned().unwrap_or_default();
        }
    }
    if record["uploadId"].is_null() {
        let body = serde_json::to_vec(
            &json!({"size":bytes.len(),"sha256":record["sha256"],"shareId":record["id"]}),
        )
        .map_err(|_| "Snapshot unavailable")?;
        let begun = request("POST", "v1/shares", &body)?;
        record["id"] = begun["id"].clone();
        record["uploadId"] = begun["uploadId"].clone();
        persist(path, record)?;
    }
    let id = record["id"].as_str().ok_or("Invalid upload response")?;
    let upload = record["uploadId"]
        .as_str()
        .ok_or("Invalid upload response")?;
    for (i, chunk) in bytes.chunks(512 * 1024).enumerate() {
        if !received.contains(&json!(i)) {
            request(
                "PUT",
                &format!("v1/shares/{id}/uploads/{upload}/parts/{i}"),
                chunk,
            )?;
        }
        *state.lock().unwrap() = json!({"state":"uploading","id":id,"progress":((i*512*1024+chunk.len())as f64/bytes.len()as f64),"missing":record["bundle"]["missing"]});
    }
    let published = request(
        "POST",
        &format!("v1/shares/{id}/uploads/{upload}/publish"),
        b"{}",
    )?;
    let url = format!("{origin}share/{id}");
    *state.lock().unwrap() = json!({"state":"shared","id":id,"url":url,"snapshot":published["snapshot"],"sha256":record["sha256"],"sourceHash":record["sourceHash"]});
    record["public"] = state.lock().unwrap().clone();
    persist(path, record)?;
    Ok(())
}

#[cfg(test)]
mod upload_tests {
    use super::*;
    use std::cell::RefCell;
    #[test]
    fn retries_only_missing_chunks_and_keeps_credentials_out_of_persisted_jobs() {
        let root = std::env::temp_dir().join(format!("vellum-share-unit-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("job.json");
        let bytes = vec![1; 600000];
        let id = "s".repeat(32);
        let upload = "u".repeat(32);
        let mut record = json!({"id":id,"sha256":vellum_assets_fs::content_hash(&bytes),"bundle":{},"sourceHash":"source"});
        let state = Mutex::new(json!({}));
        let paths = RefCell::new(vec![]);
        drive_upload(&path,&mut record,&bytes,&state,"https://account.example.test/",|method,p,_|{paths.borrow_mut().push(p.to_owned());if method=="GET"{Ok(json!({"upload":{"id":upload,"sha256":vellum_assets_fs::content_hash(&bytes)},"received":[0]}))}else if p.ends_with("publish"){Ok(json!({"snapshot":{"at":123,"incomplete":false}}))}else{Ok(json!({}))}}).unwrap();
        assert!(!paths.borrow().iter().any(|p| p.ends_with("parts/0")));
        assert!(paths.borrow().iter().any(|p| p.ends_with("parts/1")));
        assert_eq!(state.lock().unwrap()["state"], "shared");
        let persisted = std::fs::read_to_string(&path).unwrap();
        assert!(!persisted.contains("token"));
        assert_eq!(
            Uploads::new(root.clone()).status("unused")["state"],
            "not-shared"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
