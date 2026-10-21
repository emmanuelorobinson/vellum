//! TEST DATA. Chrome lane: writes actual imported-layer renderings for scripts/browser-fidelity.mjs.
mod common;
use serde_json::json;
use std::{
    io::{Read, Write},
    net::TcpListener,
    path::Path,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};
use vellum::{
    permissions::Outside,
    tools::{Registry, Session, ToolCtx, ToolError},
};
use vellum_core::ports::*;
use vellum_types::{Document, Origin};
struct Allow;
impl Outside for Allow {
    fn read_file(&self, _: &Origin, _: &Path, _: u64) -> Result<Vec<u8>, ToolError> {
        unreachable!()
    }
    fn network(&self, _: &Origin, _: &str) -> Result<(), ToolError> {
        Ok(())
    }
}
#[test]
#[ignore = "chrome lane: installed Chromium, Skia source build and loopback fixture server"]
fn import_fixture_to_real_core_and_render_for_chrome_comparison() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let addr = listener.local_addr().unwrap();
    let end = Arc::new(AtomicBool::new(false));
    let stop = end.clone();
    let server = thread::spawn(move || {
        while !stop.load(Ordering::Relaxed) {
            if let Ok((mut socket, _)) = listener.accept() {
                thread::spawn(move || {
                    socket.set_nonblocking(false).unwrap();
                    socket
                        .set_read_timeout(Some(std::time::Duration::from_secs(1)))
                        .unwrap();
                    let mut request = [0; 8192];
                    let mut n = 0;
                    loop {
                        let read = socket.read(&mut request[n..]).unwrap_or(0);
                        if read == 0 {
                            return;
                        }
                        n += read;
                        if request[..n].windows(4).any(|w| w == b"\r\n\r\n") {
                            break;
                        }
                        if n == request.len() {
                            return;
                        }
                    }
                    let font = request[..n].starts_with(b"GET /InterVariable.ttf ");
                    let bytes: &[u8] = if font {
                        include_bytes!("../../adapters/text-parley/fonts/InterVariable.ttf")
                    } else {
                        include_bytes!("../../../fixtures/browser/card.html")
                    };
                    let _ = write!(
                        socket,
                        "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        if font { "font/ttf" } else { "text/html" },
                        bytes.len()
                    );
                    socket
                        .set_write_timeout(Some(std::time::Duration::from_secs(3)))
                        .unwrap();
                    socket.write_all(bytes).unwrap();
                    let _ = socket.shutdown(std::net::Shutdown::Write);
                });
            } else {
                thread::sleep(std::time::Duration::from_millis(5));
            }
        }
    });
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/browser-fidelity");
    std::fs::create_dir_all(&root).unwrap();
    let port = Rc::new(vellum_browser_cdp::ChromiumBrowser {
        executable: std::env::var_os("VELLUM_CHROME").map(Into::into),
    });
    for (name, width, height, selector) in [
        ("phone", 375, 812, None),
        ("desktop", 800, 600, None),
        ("element", 375, 812, Some("#card")),
    ] {
        let request = BrowserRequest {
            url: format!("http://{addr}/"),
            origins: vec![],
            width,
            height,
            selector: selector.map(Into::into),
            scroll: [0., 0.],
            zoom: 1.,
            snapshot: true,
        };
        let capture = port.capture(&request).unwrap();
        assert!(
            !capture
                .dom
                .as_ref()
                .unwrap()
                .notes
                .iter()
                .any(|n| n.contains("fonts did not load")),
            "fixture fonts must load for {name}"
        );
        std::fs::write(
            root.join(format!("{name}-dom.json")),
            serde_json::to_vec_pretty(&capture.dom).unwrap(),
        )
        .unwrap();
        std::fs::write(root.join(format!("{name}-chrome.png")), &capture.png).unwrap();
        let doc:Document=serde_json::from_value(json!({"meta":{"formatVersion":1,"name":"TEST DATA","createdWith":"Vellum"},"pages":[{"id":"page000001","name":"Page"}]})).unwrap();
        let mut daemon = common::Daemon::with_scene(&doc, &["app.vellum"]);
        daemon.docs.set_browser(port.clone());
        daemon.docs.set_outside(Rc::new(Allow));
        let registry = Registry::standard();
        let mut session = Session::default();
        let mut ctx = ToolCtx::new(&mut daemon.docs, &mut session);
        registry
            .call(&mut ctx, "open_document", json!({"path":"app.vellum"}))
            .unwrap();
        let imported=registry.call(&mut ctx,"import",json!({"parent":"page000001","format":"html","text":serde_json::to_string(&capture.dom.unwrap()).unwrap()})).unwrap();
        let id = imported["created"][0].as_str().unwrap();
        let image = registry
            .call(&mut ctx, "export", json!({"nodes":[id],"format":"png"}))
            .unwrap();
        let png =
            vellum_core::base64::decode(image["files"][0]["base64"].as_str().unwrap()).unwrap();
        std::fs::write(root.join(format!("{name}-vellum.png")), png).unwrap();
    }
    end.store(true, Ordering::Relaxed);
    server.join().unwrap();
}
