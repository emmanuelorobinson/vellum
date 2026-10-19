//! Native Git front end. Git supplies immutable files; core supplies merge semantics and export scenes.
//! No daemon/session/model is needed and no DocStore is mutated here.
mod snapshot;
use super::commands::Output;
use clap::{Args, Subcommand};
use serde_json::{Value, json};
use snapshot::Snapshot;
use std::{
    collections::{BTreeMap, BTreeSet},
    io,
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
};
use vellum_core::ports::FileSystem;
use vellum_types::{Document, Node};

#[derive(Args)]
pub struct DiffArgs {
    /// Exactly two commit revisions, BASE..HEAD (not a three-dot range).
    range: String,
    /// Optional repo-relative .vellum file or directory; omitted scans all designs.
    file: Option<String>,
    /// Render PNGs (the default; accepted for compatibility with scripted usage).
    #[arg(long)]
    png: bool,
    /// A new output folder. Existing folders are never replaced.
    #[arg(short, long, default_value = "vellum-diff")]
    output: PathBuf,
}
#[derive(Subcommand)]
pub enum GitAction {
    Setup,
}
#[derive(Args)]
pub struct MergeArgs {
    base: PathBuf,
    ours: PathBuf,
    theirs: PathBuf,
}
fn invalid(e: impl std::fmt::Display) -> io::Error {
    io::Error::other(e.to_string())
}
fn git(root: &Path, args: &[&str]) -> io::Result<Vec<u8>> {
    let out = Command::new("git")
        // Partial clones must fail on absent objects instead of invoking remote helpers/network fetches.
        .env("GIT_NO_LAZY_FETCH", "1")
        // The flag also makes older Git versions fail closed instead of silently ignoring an unknown env var.
        .arg("--no-lazy-fetch")
        .arg("--no-optional-locks")
        .arg("--no-pager")
        .args(args)
        .current_dir(root)
        .output()?;
    if !out.status.success() {
        return Err(invalid(format!(
            "Git failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok(out.stdout)
}
fn root() -> io::Result<PathBuf> {
    let bytes = git(&std::env::current_dir()?, &["rev-parse", "--show-toplevel"])?;
    let s = std::str::from_utf8(&bytes)
        .map_err(invalid)?
        .trim_end_matches(['\r', '\n']);
    Ok(PathBuf::from(s))
}
fn revision(root: &Path, rev: &str) -> io::Result<String> {
    let bytes = git(
        root,
        &[
            "rev-parse",
            "--verify",
            "--end-of-options",
            &format!("{rev}^{{commit}}"),
        ],
    )?;
    let sha = std::str::from_utf8(&bytes).map_err(invalid)?.trim();
    if !matches!(sha.len(), 40 | 64) || !sha.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(invalid("Git did not resolve a commit"));
    }
    Ok(sha.into())
}
const ATTRIBUTES: &str = "\n# Vellum merge driver\n*.vellum merge=vellum\n**/*.vellum/index.json merge=vellum\n**/*.vellum/pages/*.json merge=vellum\n";
// Git executes driver commands with its shell even on Windows. Quote the executable as one shell word.
fn shell_word(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

pub fn setup(out: &Output) -> io::Result<bool> {
    let root = root()?;
    let attrs = root.join(".gitattributes");
    let old = match std::fs::symlink_metadata(&attrs) {
        Ok(meta) if meta.file_type().is_symlink() || !meta.is_file() => {
            return Err(invalid(".gitattributes must be a regular file"));
        }
        Ok(_) => std::fs::read_to_string(&attrs)?,
        Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e),
    };
    let exe = std::env::current_exe()?;
    let exe = exe
        .to_str()
        .ok_or_else(|| invalid("The merge-driver executable path must be UTF-8"))?;
    if exe.contains(['%', '\n', '\r']) {
        return Err(invalid(
            "The merge-driver executable path cannot contain %, CR or LF",
        ));
    }
    let exe = if cfg!(windows) {
        let path = exe.strip_prefix(r"\\?\").unwrap_or(exe);
        if let Some(unc) = path.strip_prefix(r"UNC\") {
            format!("//{}", unc.replace('\\', "/"))
        } else {
            path.replace('\\', "/")
        }
    } else {
        exe.into()
    };
    let driver = format!("{} merge-driver -- '%O' '%A' '%B'", shell_word(&exe));
    let settings = [
        ("merge.vellum.name", "Vellum document merge"),
        ("merge.vellum.driver", driver.as_str()),
        ("merge.vellum.recursive", "binary"),
    ];
    let mut changed = !old.ends_with(ATTRIBUTES);
    for (key, value) in settings {
        let current = git(&root, &["config", "--local", "--get", key]).ok();
        if current.as_deref() != Some(format!("{value}\n").as_bytes()) {
            git(&root, &["config", "--local", "--replace-all", key, value])?;
            changed = true;
        }
    }
    if !old.ends_with(ATTRIBUTES) {
        replace(
            &attrs,
            format!("{}{ATTRIBUTES}", old.replace(ATTRIBUTES, "")).as_bytes(),
        )?;
    }
    out.result(
        "git setup",
        json!({"changed":changed,"attributes":".gitattributes","scope":"local"}),
        "Installed the Vellum merge driver for this repo",
    )?;
    Ok(true)
}
// Unlike the daemon's serialized writer, Git/setup may be invoked concurrently in a repo. Reserve the
// temporary file exclusively: neither stale data nor a repository-supplied link may be followed or removed.
fn replace(path: &Path, bytes: &[u8]) -> io::Result<()> {
    use io::Write;
    let parent = path
        .parent()
        .ok_or_else(|| invalid("Missing destination folder"))?;
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| invalid("Destination must be UTF-8"))?;
    let temp = parent.join(format!(".{name}{}", vellum_core::files::TEMP_SUFFIX));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|e| {
            invalid(format!(
                "Cannot reserve temporary file {}: {e}; inspect any stale file before retrying",
                temp.display()
            ))
        })?;
    let result = (|| {
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temp, path)?;
        vellum_assets_fs::StdFs::new(parent)
            .sync_dir("")
            .map_err(invalid)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result
}
fn decode(bytes: &[u8]) -> io::Result<Document> {
    let doc = vellum_codec_json::JsonCodec::new()
        .decode(bytes)
        .map_err(invalid)?;
    let problems = vellum_core::ops::check_document(&doc);
    if !problems.is_empty() {
        return Err(invalid(problems.join("; ")));
    }
    Ok(doc)
}
pub fn merge_driver(args: MergeArgs, out: &Output) -> io::Result<bool> {
    let base = decode(&std::fs::read(&args.base)?)?;
    let ours = decode(&std::fs::read(&args.ours)?)?;
    let theirs = decode(&std::fs::read(&args.theirs)?)?;
    let merged = vellum_core::merge::merge(&base, &ours, &theirs).map_err(invalid)?;
    let bytes = vellum_codec_json::JsonCodec::new()
        .encode(&merged.document)
        .map_err(invalid)?;
    let meta = std::fs::symlink_metadata(&args.ours)?;
    if !meta.is_file() || meta.file_type().is_symlink() {
        return Err(invalid("The current merge input must be a regular file"));
    }
    let full = std::path::absolute(&args.ours)?;
    replace(&full, &bytes)?;
    let conflicts: Vec<_> = merged.conflicts.iter().map(|c| format!("{c:?}")).collect();
    let clean = conflicts.is_empty();
    if out.json {
        println!(
            "{}",
            json!({"ok":clean,"command":"merge-driver","conflicts":conflicts})
        );
    } else if clean {
        out.result("merge-driver", json!({}), "Merged Vellum document")?;
    } else {
        eprintln!(
            "! Vellum merge left {} conflict(s); resolve them before git add",
            conflicts.len()
        );
    }
    Ok(clean)
}

struct Target {
    signature: Value,
    name: String,
}
fn targets(doc: &Document) -> BTreeMap<String, Target> {
    fn walk(nodes: &[Node], scope: &[Value], out: &mut BTreeMap<String, Target>) {
        for (index, n) in nodes.iter().enumerate() {
            if matches!(n, Node::Frame(_)) {
                out.insert(
                    n.id().to_string(),
                    Target {
                        signature: json!({"node":n,"scope":scope,"index":index}),
                        name: n.name().into(),
                    },
                );
            } else {
                let mut parent = json!(n);
                parent
                    .as_object_mut()
                    .expect("node object")
                    .remove("children");
                let mut scope = scope.to_vec();
                scope.push(json!({"node":parent,"index":index}));
                walk(n.children(), &scope, out);
            }
        }
    }
    let mut out = BTreeMap::new();
    for (index, p) in doc.pages.iter().enumerate() {
        walk(
            &p.children,
            &[json!({"id":p.id,"name":p.name,"kind":p.kind,"index":index})],
            &mut out,
        );
    }
    for c in doc.components.values() {
        out.insert(
            c.root.id().to_string(),
            Target {
                signature: json!(c),
                name: c.name.clone(),
            },
        );
    }
    out
}
fn load(fs: Arc<dyn FileSystem>, path: &str) -> io::Result<Document> {
    let loaded = crate::daemon::DocFile {
        fs,
        path: path.into(),
        codec: Arc::new(vellum_codec_json::JsonCodec::new()),
    }
    .read()
    .map_err(invalid)?;
    if loaded.interrupted || !loaded.orphans.is_empty() {
        return Err(invalid(format!(
            "{path}: interrupted save or orphan page files"
        )));
    }
    let problems = vellum_core::ops::check_document(&loaded.document);
    if !problems.is_empty() {
        return Err(invalid(format!("{path}: {}", problems.join("; "))));
    }
    Ok(loaded.document)
}
// Select by document bytes and transitive library/asset trees before validating/rendering. An unrelated
// broken fixture cannot block a PR; a changed or referenced broken document still fails explicitly.
fn affected(
    before: &Snapshot,
    after: &Snapshot,
    path: &str,
    seen: &mut BTreeSet<String>,
) -> io::Result<bool> {
    if !seen.insert(path.into()) {
        return Ok(false);
    }
    if seen.len() > 1000 {
        return Err(invalid("More than 1,000 transitive library dependencies"));
    }
    if !before.same_path(after, path)
        || !before.same_path(after, &vellum_assets_fs::assets_dir_for(path))
    {
        return Ok(true);
    }
    let bytes = before
        .read(path)
        .or_else(|_| before.read(&format!("{path}/index.json")));
    let Ok(doc) = bytes.and_then(|b| {
        vellum_codec_json::JsonCodec::new()
            .decode(&b)
            .map_err(|e| vellum_core::ports::FsError::Unavailable(e.to_string()))
    }) else {
        return Ok(false);
    };
    for relative in doc.imports.values() {
        let import =
            vellum_core::resolver::libraries::import_path(path, relative).map_err(invalid)?;
        vellum_core::ports::check_path(&import, false).map_err(invalid)?;
        if affected(before, after, &import, seen)? {
            return Ok(true);
        }
    }
    Ok(false)
}
fn render(
    doc: &Document,
    fs: Arc<dyn FileSystem>,
    path: &str,
    id: &str,
) -> io::Result<Option<Vec<u8>>> {
    let services =
        crate::daemon::production::export_services(doc, fs.clone(), path).map_err(invalid)?;
    let scene = match crate::export_scene::prepare(doc, fs, path, &*services, &[id.into()]) {
        Ok(scene) => scene,
        // Every id here was selected from this document. The shared scene omits hidden targets, including
        // variable-bound visibility and hidden ancestors. Record that state instead of inventing pixels.
        Err(error) if error.to_string() == "No visible export target" => return Ok(None),
        Err(error) => return Err(invalid(error)),
    };
    scene.check(1.).map_err(invalid)?;
    services
        .render_export(&scene.frames, "png")
        .map(Some)
        .map_err(invalid)
}
// A failing render cannot leave a success manifest or an incomplete artifact. Only an exclusively created
// output folder is removed. Existing output (including symlinks) is refused by create_dir.
struct OutputFolder {
    path: PathBuf,
    complete: bool,
}
impl Drop for OutputFolder {
    fn drop(&mut self) {
        if !self.complete {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}
fn line(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}
pub fn diff(args: DiffArgs, out: &Output) -> io::Result<bool> {
    let (base, head) = args
        .range
        .split_once("..")
        .filter(|(b, h)| !b.is_empty() && !h.is_empty() && !h.contains("..") && !h.starts_with('.'))
        .ok_or_else(|| invalid("Use exactly two commit revisions: BASE..HEAD"))?;
    let root = root()?;
    let base = revision(&root, base)?;
    let head = revision(&root, head)?;
    let before = Arc::new(Snapshot::new(&root, &base)?);
    let after = Arc::new(Snapshot::new(&root, &head)?);
    let before_paths = before.documents()?;
    let after_paths = after.documents()?;
    let all: BTreeSet<_> = before_paths.union(&after_paths).cloned().collect();
    let paths = if let Some(file) = &args.file {
        vellum_core::ports::check_path(file, false).map_err(invalid)?;
        if !all.contains(file) {
            return Err(invalid(format!(
                "{file}: no committed Vellum document at either revision"
            )));
        }
        BTreeSet::from([file.clone()])
    } else {
        all
    };
    std::fs::create_dir(&args.output)?;
    let mut folder = OutputFolder {
        path: args.output.clone(),
        complete: false,
    };
    let mut frames = vec![];
    for file in paths {
        if args.file.is_none() && !affected(&before, &after, &file, &mut BTreeSet::new())? {
            continue;
        }
        let old = before_paths
            .contains(&file)
            .then(|| load(before.clone(), &file))
            .transpose()?;
        let new = after_paths
            .contains(&file)
            .then(|| load(after.clone(), &file))
            .transpose()?;
        let old_targets = old.as_ref().map(targets).unwrap_or_default();
        let new_targets = new.as_ref().map(targets).unwrap_or_default();
        let ids: BTreeSet<_> = old_targets
            .keys()
            .chain(new_targets.keys())
            .cloned()
            .collect();
        for id in ids {
            let b = old_targets.get(&id);
            let a = new_targets.get(&id);
            let old_png = b
                .map(|_| {
                    render(
                        old.as_ref().expect("old target"),
                        before.clone(),
                        &file,
                        &id,
                    )
                })
                .transpose()
                .map_err(|e| invalid(format!("{file} {id} before: {e}")))?
                .flatten();
            let new_png = a
                .map(|_| render(new.as_ref().expect("new target"), after.clone(), &file, &id))
                .transpose()
                .map_err(|e| invalid(format!("{file} {id} after: {e}")))?
                .flatten();
            if b.map(|t| &t.signature) == a.map(|t| &t.signature) && old_png == new_png {
                continue;
            }
            if frames.len() >= 1000 {
                return Err(invalid(
                    "Diff exceeds 1,000 changed frames; filter by document",
                ));
            }
            let ordinal = frames.len() + 1;
            let save = |png: Option<Vec<u8>>, side: &str| -> io::Result<Option<String>> {
                png.map(|png| {
                    let name = format!("{ordinal:04}-{side}.png");
                    std::fs::write(args.output.join(&name), png)?;
                    Ok(name)
                })
                .transpose()
            };
            let hidden_before = b.is_some() && old_png.is_none();
            let hidden_after = a.is_some() && new_png.is_none();
            let mut entry = json!({"file":file,"id":id,"name":a.or(b).expect("target").name,"change":match(b,a){(None,_)=>"added",(_,None)=>"removed",_=>"modified"},"before":save(old_png,"before")?,"after":save(new_png,"after")?});
            if hidden_before {
                entry["beforeState"] = json!("hidden");
            }
            if hidden_after {
                entry["afterState"] = json!("hidden");
            }
            frames.push(entry);
        }
    }
    let manifest = json!({"version":1,"base":base,"head":head,"frames":frames});
    std::fs::write(
        args.output.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    folder.complete = true;
    if !out.json {
        for f in &frames {
            println!(
                "✓ {} · {} ({}) · {}",
                line(f["file"].as_str().unwrap()),
                line(f["name"].as_str().unwrap()),
                f["id"].as_str().unwrap(),
                f["change"].as_str().unwrap()
            );
        }
    }
    let n = frames.len();
    let mut result = manifest;
    result["output"] = json!(args.output);
    out.result(
        "diff",
        result,
        &format!(
            "{n} changed frame{} · {}",
            if n == 1 { "" } else { "s" },
            args.output.join("manifest.json").display()
        ),
    )?;
    Ok(true)
}
