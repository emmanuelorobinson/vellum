//! Directory format: a folder with one file per page and an index (guide Prompt 24, ARCHITECTURE §4.3).
//!
//! ```text
//! app.vellum/
//!   index.json          the document without its pages' content: meta, imports, themes, variables, styles,
//!                       fonts, components, and every page as a stub (id, name, kind) in order
//!   pages/<page-id>.json   one page, as a document with just that page
//!   .saving             present only while a save is running, or after one was interrupted
//! ```
//!
//! Every file is an ordinary single-file `.vellum` document written by the injected `Codec`, so quantization,
//! migrations and error positions are the codec's, any tool that reads `.vellum` reads one page, and switching
//! between the formats loses nothing. A page file's meta is fixed (the page's name, the current format version),
//! so renaming the document rewrites the index only, and a changed page rewrites its own file only.
//!
//! A save that changes one file is one atomic write. A save that changes several writes `.saving`, then the pages
//! that changed, then the index, then removes pages the index no longer names, then the marker. Each file is
//! replaced whole or not at all (`atomic_write`). A crash in between cannot leave a half file, but it can leave
//! pages from different moments, and the marker is how a read knows: `Loaded::interrupted`. The next save
//! finishes the job.

use std::collections::BTreeSet;

use vellum_core::files::{TEMP_SUFFIX, atomic_write, recover_temps};
use vellum_core::ports::{
    Codec, CodecError, DocumentFiles, FileError, FileSystem, FsError, Loaded, Saved, check_path,
};
use vellum_types::{Document, DocumentKind, Page};

/// The Cargo package name, used by the smoke test and by diagnostics.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

/// The index file.
pub const INDEX: &str = "index.json";
/// The folder of page files.
pub const PAGES: &str = "pages";
/// Exists while a save runs, and stays if it is cut short.
pub const SAVING: &str = ".saving";

/// What a page file's `meta.createdWith` says.
const PAGE_CREATED_WITH: &str = "vellum-json-dir";

/// The directory format over a single-file codec.
pub struct DirCodec<'a> {
    codec: &'a dyn Codec,
}

impl<'a> DirCodec<'a> {
    /// The directory format whose files `codec` writes and reads. The composition root passes the same codec
    /// it uses for single files, so the two formats agree on every byte inside a file.
    pub fn new(codec: &'a dyn Codec) -> Self {
        Self { codec }
    }
}

fn invalid(path: impl Into<String>, message: impl Into<String>) -> FileError {
    FileError::Codec(CodecError::Invalid {
        path: path.into(),
        position: None,
        message: message.into(),
    })
}

/// Says which file an error came from. Its line and column stay the file's own, as DESIGN S05 shows them.
fn in_file(error: CodecError, file: &str) -> FileError {
    let prefix = |message: String| format!("in {file}: {message}");
    FileError::Codec(match error {
        CodecError::Syntax { position, message } => CodecError::Syntax {
            position,
            message: prefix(message),
        },
        CodecError::Invalid {
            path,
            position,
            message,
        } => CodecError::Invalid {
            path,
            position,
            message: prefix(message),
        },
        CodecError::Migration { from, message } => CodecError::Migration {
            from,
            message: prefix(message),
        },
        other => other,
    })
}

fn page_file(page: &Page) -> String {
    format!("{PAGES}/{}.json", page.id)
}

/// Names a disk would merge or refuse: ids that differ only in case (macOS and Windows keep one file for both)
/// and the names Windows reserves for devices.
fn check_page_names(doc: &Document) -> Result<(), FileError> {
    let mut seen: Vec<(String, String)> = Vec::new();
    for (index, page) in doc.pages.iter().enumerate() {
        let id = page.id.as_str();
        let lower = id.to_ascii_lowercase();
        let reserved = matches!(lower.as_str(), "con" | "prn" | "aux" | "nul")
            || ["com", "lpt"].iter().any(|p| {
                lower
                    .strip_prefix(p)
                    .is_some_and(|n| matches!(n.as_bytes(), [b'1'..=b'9']))
            });
        if reserved {
            return Err(invalid(
                format!("pages[{index}].id"),
                format!(
                    "the page id {id} is a name Windows reserves for devices, so it cannot be a file name"
                ),
            ));
        }
        if let Some((_, other)) = seen.iter().find(|(l, _)| *l == lower) {
            return Err(invalid(
                format!("pages[{index}].id"),
                if other == id {
                    format!("the page id {id} is used twice")
                } else {
                    format!(
                        "the page ids {other} and {id} differ only in case, so one disk would keep one file for both"
                    )
                },
            ));
        }
        seen.push((lower, id.to_owned()));
    }
    Ok(())
}

/// The index: everything but the pages' content.
fn index_of(doc: &Document) -> Document {
    let mut index = doc.clone();
    for page in &mut index.pages {
        page.children.clear();
    }
    index
}

/// One page as a document of its own.
fn page_document(doc: &Document, page: &Page) -> Document {
    let mut one = Document::new(page.name.clone());
    one.meta.format_version = doc.meta.format_version;
    one.meta.created_with = PAGE_CREATED_WITH.to_owned();
    one.meta.kind = DocumentKind::Document;
    one.pages = vec![page.clone()];
    one
}

fn is_temp(name: &str) -> bool {
    name.starts_with('.') && name.ends_with(TEMP_SUFFIX)
}

impl DocumentFiles for DirCodec<'_> {
    fn id(&self) -> &'static str {
        "json-dir"
    }

    fn read(&self, fs: &dyn FileSystem, path: &str) -> Result<Loaded, FileError> {
        check_path(path, false)?;
        let index_path = format!("{path}/{INDEX}");
        let index_bytes = match fs.read(&index_path) {
            Ok(bytes) => bytes,
            Err(FsError::NotFound(_)) if fs.exists(path)? => {
                return Err(invalid(
                    "",
                    format!("this folder has no {INDEX}, so it is not a Vellum document folder"),
                ));
            }
            Err(FsError::NotFound(_)) => return Err(FsError::NotFound(path.to_owned()).into()),
            Err(other) => return Err(other.into()),
        };
        let mut doc = self
            .codec
            .decode(&index_bytes)
            .map_err(|e| in_file(e, INDEX))?;

        let mut expected = BTreeSet::new();
        for (position, stub) in doc.pages.iter_mut().enumerate() {
            let file = page_file(stub);
            if !expected.insert(format!("{}.json", stub.id)) {
                return Err(invalid(
                    format!("pages[{position}]"),
                    format!("the index names {file} twice"),
                ));
            }
            let bytes = match fs.read(&format!("{path}/{file}")) {
                Ok(bytes) => bytes,
                Err(FsError::NotFound(_)) => {
                    return Err(invalid(
                        format!("pages[{position}]"),
                        format!("the page file {file} is missing"),
                    ));
                }
                Err(other) => return Err(other.into()),
            };
            let mut one = self.codec.decode(&bytes).map_err(|e| in_file(e, &file))?;
            if one.pages.len() != 1 || one.pages[0].id != stub.id {
                return Err(invalid(
                    format!("pages[{position}]"),
                    format!("{file} does not hold the page {} the index names", stub.id),
                ));
            }
            *stub = one.pages.remove(0);
        }

        let mut orphans = Vec::new();
        match fs.list(&format!("{path}/{PAGES}")) {
            Ok(names) => orphans.extend(
                names
                    .into_iter()
                    .filter(|n| !expected.contains(n) && !is_temp(n))
                    .map(|n| format!("{PAGES}/{n}")),
            ),
            Err(FsError::NotFound(_)) => {}
            Err(other) => return Err(other.into()),
        }
        Ok(Loaded {
            document: doc,
            orphans,
            interrupted: fs.exists(&format!("{path}/{SAVING}"))?,
        })
    }

    fn write(&self, fs: &dyn FileSystem, path: &str, doc: &Document) -> Result<Saved, FileError> {
        check_path(path, false)?;
        check_page_names(doc)?;

        // Everything the folder should hold, encoded before the disk is touched, so a document that will not
        // encode fails with nothing written.
        let mut plan: Vec<(String, Vec<u8>)> = Vec::with_capacity(doc.pages.len() + 1);
        for page in &doc.pages {
            let file = page_file(page);
            let bytes = self
                .codec
                .encode(&page_document(doc, page))
                .map_err(|e| in_file(e, &file))?;
            plan.push((file, bytes));
        }
        let index = (
            INDEX.to_owned(),
            self.codec
                .encode(&index_of(doc))
                .map_err(|e| in_file(e, INDEX))?,
        );
        plan.push(index);

        let mut saved = Saved::default();
        let dirty: Vec<&(String, Vec<u8>)> = plan
            .iter()
            .filter(|(file, bytes)| {
                let same = fs
                    .read(&format!("{path}/{file}"))
                    .is_ok_and(|current| &current == bytes);
                if same {
                    saved.unchanged += 1;
                }
                !same
            })
            .collect();

        let wanted: BTreeSet<String> = doc.pages.iter().map(|p| format!("{}.json", p.id)).collect();
        let pages_dir = format!("{path}/{PAGES}");
        let stale: Vec<String> = match fs.list(&pages_dir) {
            Ok(names) => names
                .into_iter()
                .filter(|n| n.ends_with(".json") && !n.starts_with('.') && !wanted.contains(n))
                .collect(),
            Err(FsError::NotFound(_)) => Vec::new(),
            Err(other) => return Err(other.into()),
        };

        // The folder is there whatever the page count, made once; a later save does not pay for it again.
        if !dirty.is_empty() && !fs.exists(&pages_dir)? {
            fs.create_dir_all(&pages_dir)?;
        }
        // One changed file is already one atomic write. Several are not, so a marker goes down first, durably:
        // from then on a crash is reported as one.
        if dirty.len() >= 2 {
            let marker = format!("{path}/{SAVING}");
            if !fs.exists(&marker)? {
                atomic_write(fs, &marker, b"")?;
            }
        }
        // Pages first, the index last (it is last in the plan): until it is written the old index still names
        // the old pages.
        for (file, bytes) in dirty {
            atomic_write(fs, &format!("{path}/{file}"), bytes)?;
            saved.written.push(file.clone());
        }
        for name in stale {
            fs.remove_file(&format!("{pages_dir}/{name}"))?;
            saved.removed.push(format!("{PAGES}/{name}"));
        }
        recover_temps(fs, &pages_dir)?;
        recover_temps(fs, path)?;
        // Only when it is there: a save that has nothing to do must not touch the disk.
        let marker = format!("{path}/{SAVING}");
        if fs.exists(&marker)? {
            fs.remove_file(&marker)?;
            fs.sync_dir(path)?;
        }
        saved.removed.sort();
        Ok(saved)
    }
}
