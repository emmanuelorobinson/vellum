// A commit's fields are private, so it cannot be written out by hand.
use vellum_core::ports::{ChangeSet, Commit};
use vellum_types::Origin;

fn forge(origin: Origin, changes: ChangeSet) -> Commit {
    Commit {
        origin,
        edits: Vec::new(),
        inverse: Vec::new(),
        changes,
    }
}

fn main() {}
