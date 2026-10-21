//! TEST DATA. Chrome lane: needs VELLUM_CHROME and permission to launch a local browser/test listener.
use std::{
    io::{Read, Write},
    net::TcpListener,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};
use vellum_browser_cdp::ChromiumBrowser;
use vellum_core::ports::{BrowserError, BrowserPort, BrowserRequest};
#[test]
#[ignore = "chrome lane: installed Chromium and loopback test listener"]
fn captures_localhost_dom_element_and_device_sizes() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let addr = listener.local_addr().unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let end = stop.clone();
    let server = thread::spawn(move || {
        while !end.load(Ordering::Relaxed) {
            if let Ok((mut socket, _)) = listener.accept() {
                let mut b = [0; 8192];
                let _ = socket.read(&mut b);
                let body = "<!doctype html><!-- TEST DATA --><style>html,body{margin:0;background:white}#card{width:240px;height:160px;background:rgb(20,40,60);border-radius:8px}p{margin:16px;color:white;font:20px/24px sans-serif}</style><article id='card'><p>Editable text</p><img width='32' height='32' src='data:image/svg+xml,%3Csvg xmlns=%22http://www.w3.org/2000/svg%22 width=%2232%22 height=%2232%22%3E%3Crect width=%2232%22 height=%2232%22 fill=%22red%22/%3E%3C/svg%3E'></article>";
                let _ = write!(
                    socket,
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
            } else {
                thread::sleep(std::time::Duration::from_millis(5));
            }
        }
    });
    let port = ChromiumBrowser {
        executable: std::env::var_os("VELLUM_CHROME").map(PathBuf::from),
    };
    let mut r = BrowserRequest {
        url: format!("http://{addr}/"),
        origins: vec![],
        width: 390,
        height: 844,
        selector: None,
        scroll: [0., 0.],
        zoom: 1.,
        snapshot: true,
    };
    let first = port.capture(&r);
    if let Err(error) = &first {
        stop.store(true, Ordering::Relaxed);
        server.join().unwrap();
        panic!("{error}");
    }
    let first = first.unwrap();
    assert_eq!((first.width, first.height), (390, 844));
    let dom = first.dom.unwrap();
    assert!(
        dom.nodes
            .iter()
            .any(|n| n.text.as_deref() == Some("Editable text"))
    );
    assert!(
        dom.nodes
            .iter()
            .any(|n| n.kind == "image" && n.image.is_some())
    );
    r.selector = Some("#card".into());
    let element = port.capture(&r).unwrap();
    assert_eq!((element.width, element.height), (240, 160));
    r.selector = Some("#missing".into());
    assert_eq!(port.capture(&r).err(), Some(BrowserError::MissingElement));
    r.selector = None;
    r.width = 800;
    r.height = 600;
    vellum_contracts::suites::browser::browser(&port, &r);
    r.width = 375;
    r.height = 812;
    r.zoom = 0.8;
    let phone = port.capture(&r).unwrap();
    assert_eq!(
        u32::from_be_bytes(phone.png[16..20].try_into().unwrap()),
        375
    );
    assert_eq!(
        u32::from_be_bytes(phone.png[20..24].try_into().unwrap()),
        812
    );
    stop.store(true, Ordering::Relaxed);
    server.join().unwrap();
}
