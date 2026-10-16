//! The session store contract: the fake passes it, and stores that break a rule fail it. TEST DATA.
mod sessions {
    use vellum_contracts::fakes::MemorySessions;
    use vellum_contracts::suites;
    use vellum_core::ports::*;
    use vellum_types::SessionId;

    #[test]
    fn the_memory_store_passes() {
        suites::sessions::run(MemorySessions::new);
    }

    /// The fake with one rule broken.
    #[derive(Default)]
    struct Broken {
        inner: MemorySessions,
        rule: &'static str,
    }
    impl SessionStore for Broken {
        fn create(&self, session: NewSession) -> Result<(), StoreError> {
            self.inner.create(session)
        }
        fn sessions(&self, doc: &DocKey) -> Result<Vec<SessionMeta>, StoreError> {
            let mut list = self.inner.sessions(doc)?;
            if self.rule == "oldest first" {
                list.reverse();
            }
            Ok(list)
        }
        fn session(&self, id: &SessionId) -> Result<Option<SessionMeta>, StoreError> {
            self.inner.session(id)
        }
        fn rename(&self, id: &SessionId, title: &str) -> Result<(), StoreError> {
            self.inner.rename(id, title)
        }
        fn append(&self, session: &SessionId, mut event: NewEvent) -> Result<EventSeq, StoreError> {
            // Accepts a plan in an external session without storing it.
            if self.rule == "records everything"
                && matches!(event.body, EventBody::Plan { .. })
                && let Some(meta) = self.inner.session(session)?
                && matches!(meta.kind, SessionKind::External { .. })
            {
                return Ok(EventSeq(99));
            }
            if self.rule == "drops thumbnails"
                && let EventBody::Step(step) = &mut event.body
            {
                step.thumbnail = None;
            }
            self.inner.append(session, event)
        }
        fn events(
            &self,
            session: &SessionId,
            since: Option<EventSeq>,
        ) -> Result<Vec<SessionEvent>, StoreError> {
            let all = self.inner.events(session, None)?;
            if self.rule == "ignores since" {
                return Ok(all);
            }
            self.inner.events(session, since)
        }
        fn put_image(&self, bytes: &[u8]) -> Result<ImageRef, StoreError> {
            self.inner.put_image(bytes)
        }
        fn image(&self, image: &ImageRef) -> Result<Option<Vec<u8>>, StoreError> {
            self.inner.image(image)
        }
        fn delete(&self, id: &SessionId) -> Result<(), StoreError> {
            if self.rule == "keeps everything" {
                return Ok(());
            }
            if self.rule == "keeps pictures" {
                let keep: Vec<_> = self
                    .inner
                    .events(id, None)
                    .unwrap_or_default()
                    .iter()
                    .flat_map(|e| e.body.images().into_iter().cloned())
                    .filter_map(|image| self.inner.image(&image).ok().flatten())
                    .collect();
                self.inner.delete(id)?;
                for bytes in keep {
                    self.inner.put_image(&bytes)?;
                }
                return Ok(());
            }
            self.inner.delete(id)
        }
    }

    #[test]
    fn broken_stores_fail() {
        for rule in [
            "oldest first",
            "records everything",
            "drops thumbnails",
            "ignores since",
            "keeps everything",
            "keeps pictures",
        ] {
            let caught = std::panic::catch_unwind(|| {
                suites::sessions::run(|| Broken {
                    inner: MemorySessions::new(),
                    rule,
                })
            });
            assert!(caught.is_err(), "the suite accepted a store that {rule}");
        }
    }
}
