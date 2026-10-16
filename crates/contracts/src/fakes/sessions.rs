//! An in-memory session store (ARCHITECTURE §3.4 lists it as `memory`). Events go through the same JSON codec
//! the SQLite store uses, so a fake log reads back exactly as a real one would.
use std::collections::BTreeMap;
use std::sync::Mutex;
use vellum_core::ports::{
    DocKey, EventSeq, ImageRef, NewEvent, NewSession, SessionEvent, SessionMeta, SessionStore,
    StoreError,
};
use vellum_core::sessions::{body_from_json, body_to_json, check_event};
use vellum_types::SessionId;

#[derive(Default)]
struct State {
    sessions: BTreeMap<SessionId, SessionMeta>,
    events: BTreeMap<SessionId, Vec<SessionEvent>>,
    images: BTreeMap<ImageRef, Vec<u8>>,
}

#[derive(Default)]
pub struct MemorySessions {
    state: Mutex<State>,
}

impl MemorySessions {
    pub fn new() -> Self {
        Self::default()
    }
    /// How many pictures the cache holds.
    pub fn cached_images(&self) -> usize {
        self.state.lock().unwrap().images.len()
    }
}

/// A stand-in content hash (FNV-1a, four lanes): 64 hex digits, stable, NOT SHA-256. Real stores use SHA-256.
fn fake_hash(bytes: &[u8]) -> ImageRef {
    let mut out = String::new();
    for lane in 0u64..4 {
        let mut h = 0xcbf2_9ce4_8422_2325u64 ^ lane.wrapping_mul(0x9e37_79b9_7f4a_7c15);
        for &b in bytes {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        out.push_str(&format!("{h:016x}"));
    }
    ImageRef::new(out).expect("64 hex digits")
}

impl SessionStore for MemorySessions {
    fn create(&self, session: NewSession) -> Result<(), StoreError> {
        let mut state = self.state.lock().unwrap();
        if state.sessions.contains_key(&session.id) {
            return Err(StoreError::Exists);
        }
        state.events.insert(session.id.clone(), vec![]);
        state.sessions.insert(
            session.id.clone(),
            SessionMeta {
                id: session.id,
                doc: session.doc,
                kind: session.kind,
                agent: session.agent,
                title: session.title,
                created_at: session.created_at,
                updated_at: session.created_at,
                events: 0,
            },
        );
        Ok(())
    }

    fn sessions(&self, doc: &DocKey) -> Result<Vec<SessionMeta>, StoreError> {
        let state = self.state.lock().unwrap();
        let mut list: Vec<_> = state
            .sessions
            .values()
            .filter(|meta| &meta.doc == doc)
            .cloned()
            .collect();
        list.sort_by(|a, b| b.updated_at.cmp(&a.updated_at).then(b.id.cmp(&a.id)));
        Ok(list)
    }

    fn session(&self, id: &SessionId) -> Result<Option<SessionMeta>, StoreError> {
        Ok(self.state.lock().unwrap().sessions.get(id).cloned())
    }

    fn rename(&self, id: &SessionId, title: &str) -> Result<(), StoreError> {
        let mut state = self.state.lock().unwrap();
        let meta = state
            .sessions
            .get_mut(id)
            .ok_or(StoreError::UnknownSession)?;
        meta.title = title.to_owned();
        Ok(())
    }

    fn append(&self, session: &SessionId, event: NewEvent) -> Result<EventSeq, StoreError> {
        let mut state = self.state.lock().unwrap();
        let meta = state
            .sessions
            .get(session)
            .ok_or(StoreError::UnknownSession)?;
        check_event(session, &meta.kind, &event)?;
        if let Some(missing) = event
            .body
            .images()
            .into_iter()
            .find(|image| !state.images.contains_key(*image))
        {
            return Err(StoreError::MissingImage(missing.clone()));
        }
        // Through the codec, as a real store would.
        let body = body_from_json(&body_to_json(&event.body))?;
        let events = state
            .events
            .get_mut(session)
            .expect("created with the session");
        let seq = EventSeq(events.len() as u64 + 1);
        events.push(SessionEvent {
            seq,
            at: event.at,
            origin: event.origin,
            body,
        });
        let meta = state.sessions.get_mut(session).expect("checked above");
        meta.events = seq.0;
        meta.updated_at = meta.updated_at.max(event.at);
        Ok(seq)
    }

    fn events(
        &self,
        session: &SessionId,
        since: Option<EventSeq>,
    ) -> Result<Vec<SessionEvent>, StoreError> {
        let state = self.state.lock().unwrap();
        let events = state
            .events
            .get(session)
            .ok_or(StoreError::UnknownSession)?;
        let after = since.map_or(0, |s| s.0);
        Ok(events.iter().filter(|e| e.seq.0 > after).cloned().collect())
    }

    fn put_image(&self, bytes: &[u8]) -> Result<ImageRef, StoreError> {
        let name = fake_hash(bytes);
        self.state
            .lock()
            .unwrap()
            .images
            .entry(name.clone())
            .or_insert_with(|| bytes.to_vec());
        Ok(name)
    }

    fn image(&self, image: &ImageRef) -> Result<Option<Vec<u8>>, StoreError> {
        Ok(self.state.lock().unwrap().images.get(image).cloned())
    }

    fn delete(&self, id: &SessionId) -> Result<(), StoreError> {
        let mut state = self.state.lock().unwrap();
        state.sessions.remove(id);
        let Some(removed) = state.events.remove(id) else {
            return Ok(());
        };
        let named = |events: &[SessionEvent], image: &ImageRef| {
            events.iter().any(|e| e.body.images().contains(&image))
        };
        let orphans: Vec<ImageRef> = removed
            .iter()
            .flat_map(|e| e.body.images())
            .filter(|image| !state.events.values().any(|events| named(events, image)))
            .cloned()
            .collect();
        for image in orphans {
            state.images.remove(&image);
        }
        Ok(())
    }
}
