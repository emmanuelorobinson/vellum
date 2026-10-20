//! Redis connection membership. Durable document broadcasts use UpdateLog, never lossy Redis pub/sub.
use std::{sync::Mutex, time::Duration};
use vellum_core::ports::sync::*;
use vellum_types::{Actor, sync::RoomId};
pub struct RedisPresence(Mutex<redis::Connection>);
fn unavailable(_: redis::RedisError) -> Error {
    Error::Unavailable
}
fn keys(room: &RoomId) -> (String, String) {
    // Hash tags keep both keys in one Redis cluster slot.
    (
        format!("vellum:sync:{{{}}}:leases", room.as_str()),
        format!("vellum:sync:{{{}}}:actors", room.as_str()),
    )
}
const PRUNE: &str = r#"
local expired = redis.call('ZRANGEBYSCORE', KEYS[1], '-inf', ARGV[1])
for _, id in ipairs(expired) do
    redis.call('ZREM', KEYS[1], id)
    redis.call('HDEL', KEYS[2], id)
end
"#;
const TOUCH: &str = r#"
if redis.call('ZCARD', KEYS[1]) >= 1024 and not redis.call('ZSCORE', KEYS[1], ARGV[2]) then return 0 end
redis.call('HSET', KEYS[2], ARGV[2], ARGV[4])
redis.call('ZADD', KEYS[1], ARGV[3], ARGV[2])
redis.call('EXPIRE', KEYS[1], 120)
redis.call('EXPIRE', KEYS[2], 120)
return 1
"#;
const LIST: &str = r#"
if redis.call('ZCARD', KEYS[1]) > 1024 then return redis.error_reply('membership limit') end
local entries = redis.call('ZRANGE', KEYS[1], 0, -1, 'WITHSCORES')
local result = {}
for i = 1, #entries, 2 do
    local actor = redis.call('HGET', KEYS[2], entries[i])
    if not actor then return redis.error_reply('missing actor') end
    table.insert(result, entries[i])
    table.insert(result, actor)
    table.insert(result, entries[i+1])
end
return result
"#;
impl RedisPresence {
    pub fn connect(url: &str) -> Result<Self, Error> {
        let client = redis::Client::open(url).map_err(unavailable)?;
        let connection = client
            .get_connection_with_timeout(Duration::from_secs(5))
            .map_err(unavailable)?;
        connection
            .set_read_timeout(Some(Duration::from_secs(5)))
            .map_err(unavailable)?;
        connection
            .set_write_timeout(Some(Duration::from_secs(5)))
            .map_err(unavailable)?;
        Ok(Self(Mutex::new(connection)))
    }
}
impl Presence for RedisPresence {
    fn touch(&self, room: &RoomId, entry: &PresenceEntry, now_ms: i64) -> Result<(), Error> {
        if entry.expires_ms <= now_ms || entry.expires_ms > now_ms.saturating_add(30_000) {
            return Err(Error::InvalidUpdate);
        }
        let actor = serde_json::to_string(entry).map_err(|_| Error::InvalidUpdate)?;
        let (leases, actors) = keys(room);
        let mut connection = self.0.lock().map_err(|_| Error::Unavailable)?;
        let result: i64 = redis::cmd("EVAL")
            .arg(format!("{PRUNE}{TOUCH}"))
            .arg(2)
            .arg(leases)
            .arg(actors)
            .arg(now_ms)
            .arg(&entry.connection)
            .arg(entry.expires_ms)
            .arg(actor)
            .query(&mut *connection)
            .map_err(unavailable)?;
        if result == 1 {
            Ok(())
        } else {
            Err(Error::Limit)
        }
    }
    fn list(&self, room: &RoomId, now_ms: i64) -> Result<Vec<PresenceEntry>, Error> {
        let (leases, actors) = keys(room);
        let mut connection = self.0.lock().map_err(|_| Error::Unavailable)?;
        let values: Vec<String> = redis::cmd("EVAL")
            .arg(format!("{PRUNE}{LIST}"))
            .arg(2)
            .arg(leases)
            .arg(actors)
            .arg(now_ms)
            .query(&mut *connection)
            .map_err(unavailable)?;
        let (triples, remainder) = values.as_chunks::<3>();
        if !remainder.is_empty() {
            return Err(Error::Unavailable);
        }
        let mut entries = triples
            .iter()
            .map(|triple| {
                if let Ok(mut entry) = serde_json::from_str::<PresenceEntry>(&triple[1]) {
                    entry.connection = triple[0].clone();
                    entry.expires_ms = triple[2].parse().map_err(|_| Error::Unavailable)?;
                    return Ok(entry);
                }
                let actor: Actor =
                    serde_json::from_str(&triple[1]).map_err(|_| Error::Unavailable)?;
                Ok(PresenceEntry {
                    connection: triple[0].clone(),
                    actor,
                    expires_ms: triple[2].parse().map_err(|_| Error::Unavailable)?,
                    state: None,
                })
            })
            .collect::<Result<Vec<_>, Error>>()?;
        entries.sort_by(|a, b| a.connection.cmp(&b.connection));
        Ok(entries)
    }
    fn remove(&self, room: &RoomId, id: &str) -> Result<(), Error> {
        let (leases, actors) = keys(room);
        let mut connection = self.0.lock().map_err(|_| Error::Unavailable)?;
        redis::pipe()
            .atomic()
            .cmd("ZREM")
            .arg(leases)
            .arg(id)
            .ignore()
            .cmd("HDEL")
            .arg(actors)
            .arg(id)
            .ignore()
            .query::<()>(&mut *connection)
            .map_err(unavailable)
    }
}
