//! TEST DATA. Canary is passed only in the environment, never arguments or output.
fn main() -> std::process::ExitCode {
    let Ok(canary) = std::env::var("VELLUM_CANARY_API_KEY") else {
        eprintln!("canary: missing test credential");
        return std::process::ExitCode::FAILURE;
    };
    let roots: Vec<_> = std::env::args_os()
        .skip(1)
        .map(std::path::PathBuf::from)
        .collect();
    if roots.is_empty() {
        eprintln!("canary: no artifact roots");
        return std::process::ExitCode::FAILURE;
    }
    match vellum_testkit::model::scan_canary(&roots, canary.as_bytes()) {
        Ok(hits) if hits.is_empty() => {
            println!("canary: no persisted credential bytes");
            std::process::ExitCode::SUCCESS
        }
        Ok(hits) => {
            for path in hits {
                eprintln!("canary: leaked credential in {}", path.display());
            }
            std::process::ExitCode::FAILURE
        }
        Err(_) => {
            eprintln!("canary: scan incomplete");
            std::process::ExitCode::FAILURE
        }
    }
}
