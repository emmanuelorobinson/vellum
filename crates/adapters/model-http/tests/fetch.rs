//! TEST DATA. A one-thread server on the loopback address stands in for a stock image provider.
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::time::Duration;

use vellum_core::ports::{FetchError, FetchRequest, HttpFetch, SecretString};
use vellum_model_http::HttpFetcher;

/// Serves `responses` in turn, one per connection, and reports each request's first line.
fn serve(responses: Vec<Vec<u8>>) -> (u16, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        for response in responses {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            let mut buffer = [0u8; 4096];
            let n = stream.read(&mut buffer).unwrap_or(0);
            let head = String::from_utf8_lossy(&buffer[..n]);
            let _ = sender.send(head.lines().next().unwrap_or("").to_owned());
            let _ = stream.write_all(&response);
        }
    });
    (port, receiver)
}

fn http(status: &str, headers: &str, body: &[u8]) -> Vec<u8> {
    let mut out = format!(
        "HTTP/1.1 {status}\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    out.extend_from_slice(body);
    out
}

fn fetcher() -> HttpFetcher {
    HttpFetcher::with_timeout(Duration::from_secs(5)).allowing_loopback()
}

fn get(
    port: u16,
    path: &str,
    secret: Option<&str>,
    max: usize,
) -> Result<vellum_core::ports::FetchResponse, FetchError> {
    fetcher().get(&FetchRequest {
        url: format!("http://127.0.0.1:{port}{path}"),
        secret: secret.map(SecretString::new),
        max_bytes: max,
    })
}

#[test]
fn the_default_fetcher_refuses_bad_addresses_without_touching_the_network() {
    vellum_contracts::suites::import::http_fetch_refuses_bad_addresses(&HttpFetcher::default());
    // Loopback is only allowed when asked for, and then only plain loopback.
    assert_eq!(
        HttpFetcher::default()
            .get(&FetchRequest {
                url: "http://127.0.0.1:9/".into(),
                secret: None,
                max_bytes: 10
            })
            .err(),
        Some(FetchError::Blocked)
    );
}

#[test]
fn a_good_answer_comes_back_with_its_status_type_and_body() {
    let (port, requests) = serve(vec![http(
        "200 OK",
        "Content-Type: Image/JPEG; charset=x\r\n",
        b"pixels",
    )]);
    let r = get(port, "/a.jpg", None, 100).unwrap();
    assert_eq!(
        (r.status, r.content_type.as_str(), r.body.as_slice()),
        (200, "image/jpeg", &b"pixels"[..])
    );
    assert_eq!(requests.recv().unwrap(), "GET /a.jpg HTTP/1.1");
}

#[test]
fn the_secret_goes_into_the_request_and_nowhere_else() {
    let (port, requests) = serve(vec![http("200 OK", "", b"{}")]);
    get(port, "/api?key={secret}&q=cats", Some("s3 cret"), 100).unwrap();
    assert_eq!(
        requests.recv().unwrap(),
        "GET /api?key=s3%20cret&q=cats HTTP/1.1"
    );
}

#[test]
fn errors_statuses_are_passed_on_and_redirects_are_not_followed() {
    let (port, _) = serve(vec![
        http("429 Too Many Requests", "", b"slow down"),
        http(
            "302 Found",
            "Location: http://127.0.0.1:1/elsewhere\r\n",
            b"",
        ),
    ]);
    assert_eq!(get(port, "/", None, 100).unwrap().status, 429);
    assert_eq!(get(port, "/", None, 100).unwrap().status, 302);
}

#[test]
fn an_answer_over_the_limit_is_refused() {
    let (port, _) = serve(vec![http("200 OK", "", &[7u8; 50])]);
    assert_eq!(get(port, "/", None, 10).err(), Some(FetchError::TooLarge));
}

#[test]
fn a_server_that_is_not_there_is_a_network_error() {
    let port = {
        TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port()
    };
    assert_eq!(get(port, "/", None, 10).err(), Some(FetchError::Network));
}
