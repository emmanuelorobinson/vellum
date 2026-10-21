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

#[test]
#[ignore = "network lane: needs a loopback TCP socket, no external service"]
fn media_exchange_posts_json_with_header_credentials_and_bounds_redirects_and_bodies() {
    use vellum_core::ports::{MediaHttp, MediaMethod, MediaRequest};
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        use std::io::BufRead;
        let (stream, _) = listener.accept().unwrap();
        let mut reader = std::io::BufReader::new(stream);
        let mut head = String::new();
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if line == "\r\n" {
                break;
            }
            head.push_str(&line)
        }
        let len = head
            .lines()
            .find_map(|l| {
                l.to_ascii_lowercase()
                    .strip_prefix("content-length: ")
                    .and_then(|s| s.parse::<usize>().ok())
            })
            .unwrap_or(0);
        let mut body = vec![0; len];
        reader.read_exact(&mut body).unwrap();
        tx.send((head, body)).unwrap();
        reader
            .get_mut()
            .write_all(&http("200 OK", "Content-Type: application/json\r\n", b"{}"))
            .unwrap();
    });
    let request = MediaRequest {
        url: format!("http://127.0.0.1:{port}/generate"),
        method: MediaMethod::Post,
        body: b"{\"n\":1}".to_vec(),
        credential: Some((
            "Authorization".into(),
            SecretString::new("Bearer TEST_DATA_MEDIA_CANARY_66"),
        )),
        max_bytes: 100,
    };
    let response = fetcher().exchange(&request).unwrap();
    assert_eq!(response.body, b"{}");
    let (head, body) = rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(head.starts_with("POST /generate HTTP/1.1"));
    assert!(
        head.to_lowercase()
            .contains("authorization: bearer test_data_media_canary_66")
    );
    assert!(!head.lines().next().unwrap().contains("CANARY"));
    assert_eq!(body, request.body);
    let (port, _) = serve(vec![http(
        "302 Found",
        "Location: https://example.com/redirect\r\n",
        b"",
    )]);
    let r = MediaRequest {
        url: format!("http://127.0.0.1:{port}/"),
        method: MediaMethod::Get,
        body: vec![],
        credential: None,
        max_bytes: 2,
    };
    assert_eq!(fetcher().exchange(&r).unwrap().status, 302);
    let (port, _) = serve(vec![http("200 OK", "", b"larger")]);
    let r = MediaRequest {
        url: format!("http://127.0.0.1:{port}/"),
        ..r
    };
    assert_eq!(fetcher().exchange(&r), Err(FetchError::TooLarge));
}
