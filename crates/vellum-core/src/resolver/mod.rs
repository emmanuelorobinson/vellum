//! Variables, component instances, overrides, slots and library imports resolved into a tree for layout,
//! display and inspection. Use [`Resolver`] for the complete tree; [`variables`] also exposes the lower-level
//! variable resolver and bind-time checks.

pub mod variables;

pub mod components;
pub mod libraries;

pub use components::{ResolvedNode, Resolver};
