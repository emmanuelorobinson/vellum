//! TEST DATA. @network: loopback HTTP fixture; no external account/keychain.
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
};
use vellum_account_http::{AccountClient, AccountUrl, TokenReply};
#[test]
#[ignore = "needs loopback sockets; run in the native/cloud lane"]
fn device_wire_is_bounded_and_tokens_never_enter_the_public_user() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        for (path, reply) in [
            (
                "/api/auth/device/code",
                r#"{"device_code":"private-device","user_code":"ABCD2345","expires_in":600,"interval":5}"#,
            ),
            (
                "/api/auth/device/token",
                r#"{"access_token":"test-only-account-session"}"#,
            ),
            (
                "/v1/me",
                r#"{"user":{"id":"person","email":"person@example.test","name":"Person"},"session":{"token":"ignored-field"}}"#,
            ),
            ("/api/auth/sign-out", r#"{"success":true}"#),
        ] {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut bytes = Vec::new();
            let mut buffer = [0; 4096];
            loop {
                let n = socket.read(&mut buffer).unwrap();
                bytes.extend_from_slice(&buffer[..n]);
                if bytes.windows(4).any(|s| s == b"\r\n\r\n") {
                    break;
                }
            }
            let header = String::from_utf8_lossy(&bytes);
            assert!(header.lines().next().unwrap().contains(path));
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                reply.len(),
                reply
            );
            socket.write_all(response.as_bytes()).unwrap();
        }
    });
    let client = AccountClient::new(AccountUrl::parse(&origin).unwrap()).unwrap();
    let device = client.start().unwrap();
    assert_eq!(device.user_code, "ABCD2345");
    assert!(!device.verification_url.contains("private-device"));
    let TokenReply::Ready(token) = client.poll(&device).unwrap() else {
        panic!("token expected")
    };
    let user = client.me(&token).unwrap();
    let safe = serde_json::to_string(&user).unwrap();
    assert!(!safe.contains(token.expose()));
    assert!(!safe.contains("ignored-field"));
    client.revoke(&token).unwrap();
    server.join().unwrap();
}
