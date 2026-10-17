use std::sync::OnceLock;

use vellum_core::ports::IconLibrary;

use crate::Library;

/// The shipped sets, in the order a picker lists them.
const DATA: [(&str, &str); 6] = [
    ("lucide", include_str!("../data/lucide.json")),
    (
        "material-symbols-outlined",
        include_str!("../data/material-symbols-outlined.json"),
    ),
    (
        "material-symbols-rounded",
        include_str!("../data/material-symbols-rounded.json"),
    ),
    (
        "material-symbols-sharp",
        include_str!("../data/material-symbols-sharp.json"),
    ),
    ("phosphor", include_str!("../data/phosphor.json")),
    ("feather", include_str!("../data/feather.json")),
];

fn shipped(slot: usize) -> &'static Library {
    static SETS: [OnceLock<Library>; 6] = [
        OnceLock::new(),
        OnceLock::new(),
        OnceLock::new(),
        OnceLock::new(),
        OnceLock::new(),
        OnceLock::new(),
    ];
    SETS[slot]
        .get_or_init(|| Library::from_json(DATA[slot].1).expect("the shipped icon data reads"))
}

/// A set of libraries by id: the shipped ones, plus any the host adds (a project's own icon folder).
pub struct Catalog {
    extra: Vec<Library>,
}

impl Catalog {
    /// Only the shipped sets.
    pub fn builtin() -> Self {
        Self { extra: Vec::new() }
    }

    /// Adds a set. A set with the id of a shipped one replaces it.
    #[must_use]
    pub fn with(mut self, library: Library) -> Self {
        self.extra.retain(|l| l.id() != library.id());
        self.extra.push(library);
        self
    }

    /// The ids, in listing order: the shipped sets, then the added ones.
    pub fn ids(&self) -> Vec<String> {
        let shipped = DATA
            .iter()
            .filter(|(id, _)| !self.extra.iter().any(|l| l.id() == *id))
            .map(|(id, _)| (*id).to_owned());
        shipped
            .chain(self.extra.iter().map(|l| l.id().to_owned()))
            .collect()
    }

    /// The library called `id`. `material-symbols` is the outlined style.
    pub fn library(&self, id: &str) -> Option<&dyn IconLibrary> {
        let id = if id == "material-symbols" {
            "material-symbols-outlined"
        } else {
            id
        };
        if let Some(l) = self.extra.iter().find(|l| l.id() == id) {
            return Some(l);
        }
        DATA.iter()
            .position(|(known, _)| *known == id)
            .map(|slot| shipped(slot) as &dyn IconLibrary)
    }
}
