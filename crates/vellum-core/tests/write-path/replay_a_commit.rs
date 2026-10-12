// A store that is handed a commit cannot copy it to apply again later, or elsewhere.
use vellum_core::ports::Commit;

fn keep(commit: &Commit) -> Commit {
    commit.clone()
}

fn main() {}
