use std::{
    future::Future,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
};
use vellum_core::ports::*;

/// Minimal executor for runtime-independent port contracts; parks on pending futures.
pub fn block_on<F: Future>(future: F) -> F::Output {
    struct ThreadWake(std::thread::Thread);
    impl Wake for ThreadWake {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Waker::from(Arc::new(ThreadWake(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::park(),
        }
    }
}

/// Uses only a namespaced temporary credential. The store must not be read-only.
pub fn secrets(store: &dyn SecretStore) {
    let key = "vellum-contract-test";
    store.delete(key).expect("SecretStore: delete missing");
    assert!(
        store.get(key).unwrap().is_none(),
        "SecretStore: fabricated credential"
    );
    store
        .set(key, SecretString::new("contract-test-first"))
        .unwrap();
    assert_eq!(
        store.get(key).unwrap().unwrap().expose(),
        "contract-test-first",
        "SecretStore: round trip"
    );
    store
        .set(key, SecretString::new("contract-test-second"))
        .unwrap();
    assert_eq!(
        store.get(key).unwrap().unwrap().expose(),
        "contract-test-second",
        "SecretStore: overwrite"
    );
    store.delete(key).unwrap();
    assert!(store.get(key).unwrap().is_none(), "SecretStore: delete");
}

/// Supply a provider scripted to return at least one text delta and a terminal event.
pub fn provider(provider: &dyn ModelProvider) {
    assert!(!provider.id().is_empty(), "ModelProvider: missing identity");
    block_on(async {
        let mut stream = provider
            .complete(ChatRequest::text("test", "hi"), Cancellation::default())
            .await
            .expect("ModelProvider: completion");
        let mut text = false;
        let mut terminal = false;
        for _ in 0..100 {
            match stream.next().await {
                Some(Ok(ModelEvent::Text(_))) => text = true,
                Some(Ok(ModelEvent::Finished(_))) => {
                    terminal = true;
                    break;
                }
                Some(Ok(_)) => {}
                _ => panic!("ModelProvider: missing terminal event"),
            }
        }
        assert!(
            text && terminal,
            "ModelProvider: missing text/terminal event"
        );
        assert!(
            stream.next().await.is_none(),
            "ModelProvider: stream is not fused"
        );
    });
}

/// Supply a wire scripted with {probe:"ready"}, then [DONE]. Cancellation must not consume it.
pub fn transport(transport: &dyn ModelTransport, endpoint: &str) {
    let request = || ModelHttpRequest {
        endpoint: endpoint.into(),
        headers: vec![],
        credential: None,
        body: serde_json::json!({"probe":"ready"}),
    };
    let cancel = Cancellation::default();
    cancel.cancel();
    assert!(
        matches!(
            block_on(transport.start(request(), cancel)),
            Err(ModelError::Cancelled)
        ),
        "ModelTransport: cancelled request was started"
    );
    block_on(async {
        let mut stream = transport
            .start(request(), Cancellation::default())
            .await
            .expect("ModelTransport: start");
        assert!(
            matches!(stream.next().await, Some(Ok(WireEvent::Data(value))) if value["probe"] == "ready"),
            "ModelTransport: changed/dropped event"
        );
        assert!(
            matches!(stream.next().await, Some(Ok(WireEvent::Done))),
            "ModelTransport: lost terminal marker"
        );
        assert!(
            stream.next().await.is_none(),
            "ModelTransport: fabricated extra event"
        );
        assert!(stream.next().await.is_none(), "ModelTransport: not fused");
    });
}
