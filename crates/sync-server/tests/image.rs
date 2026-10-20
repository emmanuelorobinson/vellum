//! TEST DATA. @docker/@network: built image, Docker engine and loopback networking required.
#![cfg(unix)]
mod support;
use std::{os::unix::fs::MetadataExt, process::Command, sync::Arc};
use vellum_sync_server::SyncService;
use vellum_types::sync::*;
struct Container(String);
impl Drop for Container {
    fn drop(&mut self) {
        let _ = Command::new("docker")
            .args(["rm", "--force", &self.0])
            .output();
    }
}
#[test]
#[ignore = "@docker/@network: VELLUM_TEST_SYNC_IMAGE, built image and Docker engine required"]
fn identical_four_actor_scenario_in_self_host_image() {
    let image = std::env::var("VELLUM_TEST_SYNC_IMAGE").expect("set built test image");
    let path = std::env::temp_dir().join(format!("vellum-p64-image-{}", std::process::id()));
    let room = RoomId::new("image-four-actors").unwrap();
    let keys = support::keys();
    {
        let disk = Arc::new(vellum_sync_disk::DiskSnapshots::open(&path).unwrap());
        let database =
            Arc::new(vellum_sync_sqlite::SqliteLog::open(path.join("sync.sqlite")).unwrap());
        for (key, actor) in keys.iter().zip(support::actors()) {
            database
                .grant(
                    key,
                    &room,
                    &Grant {
                        actor,
                        role: Role::Editor,
                    },
                    i64::MAX,
                )
                .unwrap();
        }
        SyncService::new(database.clone(), disk, database)
            .seed(&room, &vellum_contracts::samples::document())
            .unwrap();
    }
    let metadata = path.metadata().unwrap();
    let name = format!("vellum-p64-image-{}", std::process::id());
    let result = Command::new("docker")
        .args([
            "run",
            "-d",
            "--rm",
            "--name",
            &name,
            "--read-only",
            "--cap-drop=ALL",
            "--security-opt=no-new-privileges:true",
            "--user",
            &format!("{}:{}", metadata.uid(), metadata.gid()),
            "-p",
            "127.0.0.1::7310",
            "-v",
            &format!("{}:/data", path.display()),
            &image,
        ])
        .output()
        .unwrap();
    assert!(result.status.success(), "image starts");
    let container = Container(name);
    let port = Command::new("docker")
        .args(["port", &container.0, "7310/tcp"])
        .output()
        .unwrap();
    let address = String::from_utf8(port.stdout)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    let mut ready = false;
    for _ in 0..30 {
        if std::net::TcpStream::connect(address).is_ok() {
            ready = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    assert!(ready, "container listener becomes ready");
    support::scenario(address, &room, &keys);
    let health = Command::new("docker")
        .args([
            "exec",
            &container.0,
            "curl",
            "--fail",
            "--silent",
            "http://127.0.0.1:7310/health",
        ])
        .output()
        .unwrap();
    assert!(health.status.success());
    assert_eq!(health.stdout, b"ok");
    drop(container);
    std::fs::remove_dir_all(path).unwrap();
}
