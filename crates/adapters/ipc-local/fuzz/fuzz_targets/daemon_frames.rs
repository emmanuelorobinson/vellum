//! Uses the production bounded incremental reader, never a listener or real credential.
#![no_main]
#[path = "../../src/framing.rs"]
mod framing;
use tokio::io::{AsyncBufReadExt, BufReader};
libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let Some((capacity, payload)) = data.split_first() else {
        return;
    };
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(async {
            let mut reader = BufReader::with_capacity(1 + usize::from(*capacity), payload);
            let mut consumed = 0;
            loop {
                let expected = payload[consumed..]
                    .iter()
                    .position(|b| *b == b'\n')
                    .map(|n| n + 1);
                match framing::frame(&mut reader, 4096).await {
                    Ok(Some(bytes)) => {
                        let n = expected.expect("only complete frames are accepted");
                        assert!(n <= 4096);
                        assert_eq!(bytes, payload[consumed..consumed + n]);
                        consumed += n;
                        let _ = serde_json::from_slice::<vellum_types::daemon::DaemonHello>(&bytes);
                    }
                    Ok(None) => {
                        assert_eq!(consumed, payload.len());
                        break;
                    }
                    Err(_) => {
                        assert!(expected.is_none_or(|n| n > 4096));
                        break;
                    }
                }
            }
            let _ = reader.fill_buf().await;
        });
});
