//! Library data crosses an injected source boundary. Paths are lexical and relative to the importing
//! file. The cache keeps the last valid library on a missing/unreadable path, with an explicit flag.
use std::collections::BTreeMap;
use std::sync::Arc;
use vellum_types::{Document, DocumentKind};

mod paste;
pub use paste::*;
#[cfg(test)]
mod tests;

/// A host reads and decodes library files; core never opens files or touches the network.
pub trait LibrarySource {
    /// `path` is a normalized document path. `None` means missing, `Err` means unreadable.
    fn load(&self, path: &str) -> Result<Option<Document>, String>;
}
/// A library lookup, including whether it came from a last-known copy.
#[derive(Debug, Clone)]
pub struct Library {
    /// Normalized path used as the cache key.
    pub path: String,
    /// A fresh library, a last-known library, or no available copy.
    pub document: Option<Arc<Document>>,
    /// The source did not return a valid fresh library.
    pub missing: bool,
    /// Why a fresh library could not be used.
    pub problem: Option<String>,
}
/// Persistent last-known copies. A host may seed these from its disk cache; persistence is Prompt 24.
pub struct Libraries<'a> {
    source: &'a dyn LibrarySource,
    cache: BTreeMap<String, Arc<Document>>,
    loaded: BTreeMap<String, Library>,
}
impl<'a> Libraries<'a> {
    /// A cache using the host's file source.
    pub fn new(source: &'a dyn LibrarySource) -> Self {
        Self {
            source,
            cache: BTreeMap::new(),
            loaded: BTreeMap::new(),
        }
    }
    /// Start a resolve pass. Each distinct path is refreshed once in that pass.
    pub fn begin(&mut self) {
        self.loaded.clear();
    }
    /// Seeds a last-known copy after a host reads its cache.
    pub fn seed(&mut self, path: &str, doc: Document) -> Result<(), String> {
        if doc.meta.kind != DocumentKind::Library {
            return Err("cached document is not a library".into());
        }
        self.cache.insert(normalize(path), Arc::new(doc));
        Ok(())
    }
    /// Last-known copies for the host to persist.
    pub fn cached(&self) -> impl Iterator<Item = (&str, &Document)> {
        self.cache.iter().map(|(p, d)| (p.as_str(), d.as_ref()))
    }
    /// Resolve an import against its owning document, refreshing from the source with cache fallback.
    pub fn get(&mut self, owner: &str, relative: &str) -> Result<Library, String> {
        let path = import_path(owner, relative)?;
        if let Some(value) = self.loaded.get(&path) {
            return Ok(value.clone());
        }
        let problem = match self.source.load(&path) {
            Ok(Some(doc)) if doc.meta.kind == DocumentKind::Library => {
                self.cache.insert(path.clone(), Arc::new(doc));
                None
            }
            Ok(Some(_)) => Some("import is not a library document".into()),
            Ok(None) => Some("library file is missing".into()),
            Err(e) => Some(e),
        };
        let value = Library {
            document: self.cache.get(&path).cloned(),
            missing: problem.is_some(),
            problem,
            path: path.clone(),
        };
        self.loaded.insert(path, value.clone());
        Ok(value)
    }
}
/// Lexical document path normalization. No filesystem access, symlink lookup or case folding.
pub fn normalize(path: &str) -> String {
    let mut parts = Vec::new();
    let absolute = path.starts_with('/');
    for p in path.split('/') {
        match p {
            "" | "." => {}
            ".." if parts.last().is_some_and(|v| *v != "..") => {
                parts.pop();
            }
            ".." if absolute => {}
            _ => parts.push(p),
        }
    }
    format!("{}{}", if absolute { "/" } else { "" }, parts.join("/"))
}
/// Resolves a relative import; accepts `..` because sibling libraries are valid.
pub fn import_path(owner: &str, relative: &str) -> Result<String, String> {
    if relative.is_empty()
        || relative.starts_with('/')
        || relative.contains(':')
        || relative.contains('\\')
        || relative.contains('\0')
    {
        return Err("a library import must be a relative path".into());
    }
    if !relative.ends_with(".lib.vellum") {
        return Err("a library import must name a .lib.vellum file".into());
    }
    let dir = owner.rsplit_once('/').map(|(dir, _)| dir).unwrap_or(".");
    Ok(normalize(&format!("{dir}/{relative}")))
}
/// A portable relative link from one document to another.
pub fn relative_path(owner: &str, target: &str) -> String {
    let owner = normalize(owner);
    let target = normalize(target);
    let a: Vec<_> = owner
        .rsplit_once('/')
        .map(|(d, _)| d)
        .unwrap_or("")
        .split('/')
        .filter(|v| !v.is_empty())
        .collect();
    let b: Vec<_> = target.split('/').filter(|v| !v.is_empty()).collect();
    let shared = a.iter().zip(&b).take_while(|(a, b)| a == b).count();
    std::iter::repeat_n("..", a.len() - shared)
        .chain(b[shared..].iter().copied())
        .collect::<Vec<_>>()
        .join("/")
}
