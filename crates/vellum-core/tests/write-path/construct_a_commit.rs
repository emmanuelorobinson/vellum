// There is no public constructor either.
use vellum_core::ports::{ChangeSet, Commit};
use vellum_types::Origin;

fn forge(origin: Origin, changes: ChangeSet) -> Commit {
    Commit::new(origin, Vec::new(), Vec::new(), changes)
}

fn main() {}
