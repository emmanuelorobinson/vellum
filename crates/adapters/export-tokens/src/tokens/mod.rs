//! The three formats and what they share.

mod css;
mod dtcg;
mod names;
mod value;

use vellum_core::ports::TokenFormat;

pub use css::{CssVars, TailwindV4};
pub use dtcg::DtcgJson;

/// Every format, in the order a person sees them.
pub fn formats() -> Vec<Box<dyn TokenFormat>> {
    vec![Box::new(CssVars), Box::new(TailwindV4), Box::new(DtcgJson)]
}

/// The format called `id` (`css-vars`, `tailwind-v4`, `dtcg-json`).
pub fn format_by_id(id: &str) -> Option<Box<dyn TokenFormat>> {
    formats().into_iter().find(|format| format.id() == id)
}
