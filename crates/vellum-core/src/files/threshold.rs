//! When to suggest the multi-file format (DESIGN S06, ADR-004).

use vellum_types::{Document, Node};

/// The node count at which the multi-file format is suggested: ADR-004's 10,000.
pub const LARGE_FILE_NODES: usize = 10_000;

/// The size of the single-file JSON at which it is suggested: ADR-004's 4 MB (decimal megabytes).
pub const LARGE_FILE_BYTES: usize = 4_000_000;

/// Whether a document this big should get a quiet suggestion to use the directory format: at either limit,
/// whichever comes first. The suggestion never blocks anything, and a document that is already a folder is not
/// asked (the caller knows which it has).
pub fn suggest_multi_file(nodes: usize, json_bytes: usize) -> bool {
    nodes >= LARGE_FILE_NODES || json_bytes >= LARGE_FILE_BYTES
}

/// Every node in the document: on its pages and inside its components.
pub fn count_nodes(doc: &Document) -> usize {
    fn count(node: &Node) -> usize {
        1 + node.children().iter().map(count).sum::<usize>()
    }
    let pages: usize = doc.pages.iter().flat_map(|p| &p.children).map(count).sum();
    let components: usize = doc.components.values().map(|c| count(&c.root)).sum();
    pages + components
}
