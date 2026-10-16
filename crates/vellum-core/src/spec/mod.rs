//! `get_spec`: a frame described for the agent that will write its code (ARCHITECTURE §3.5, §7.3).
//!
//! - [`LayoutSpecV1`] is the framework-neutral description: the layer tree in the file's own words (layout, sizing,
//!   fills, text), every variable replaced by its name (`{"$token": "color/primary"}`) with the values it takes
//!   under each theme value listed beside it, and the components and text styles the layers use.
//! - [`StackProfile`] is the hint for one stack (shadcn, React Native, SwiftUI): how the spec's words map to that
//!   stack's elements, layout and tokens. It is data, in JSON, and swappable: a person adds or replaces a stack
//!   without a new build ([`StackProfiles::with`]).

mod layout;
mod stacks;

#[cfg(test)]
mod tests;

pub use layout::LayoutSpecV1;
pub use stacks::{StackProfile, StackProfiles};
