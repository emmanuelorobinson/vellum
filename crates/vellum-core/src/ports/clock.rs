//! Time and identifiers come in through ports, so `vellum-core` never reads the wall clock or randomness.

use vellum_types::{ComponentId, NodeId, PageId, StyleId, VariableId};

/// UTC milliseconds since the Unix epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(pub i64);

/// The current time. Tests inject a fixed one.
pub trait Clock {
    /// Now. A real clock may repeat a value but never goes back.
    fn now(&self) -> Timestamp;
}

/// Mints ids for things inside a document: ten-character nanoid-style strings, unique per document and never
/// reused (guide §9).
pub trait IdGen {
    /// A fresh node id.
    fn node_id(&mut self) -> NodeId;

    /// A fresh page id.
    fn page_id(&mut self) -> PageId {
        let id = self.node_id();
        PageId::new(id.as_str()).expect("a node id is a valid page id")
    }

    /// A fresh component id.
    fn component_id(&mut self) -> ComponentId {
        let id = self.node_id();
        ComponentId::new(id.as_str()).expect("a node id is a valid component id")
    }

    /// A fresh variable id.
    fn variable_id(&mut self) -> VariableId {
        let id = self.node_id();
        VariableId::new(id.as_str()).expect("a node id is a valid variable id")
    }

    /// A fresh style id.
    fn style_id(&mut self) -> StyleId {
        let id = self.node_id();
        StyleId::new(id.as_str()).expect("a node id is a valid style id")
    }
}
