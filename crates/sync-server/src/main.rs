//! Explicit service administration. Capability and object credentials are read from the OS keychain.
use clap::{Parser, Subcommand};
use std::{net::SocketAddr, path::PathBuf, sync::Arc};
use vellum_core::ports::{SecretStore, SecretString, sync::*};
use vellum_sync_server::SyncService;
use vellum_types::{Actor, sync::*};
#[derive(Parser)]
#[command(about = "Optional document sync service")]
struct Args {
    #[arg(long, default_value = ".sync-data")]
    data: PathBuf,
    #[arg(long)]
    cloud: bool,
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Serve {
        #[arg(long)]
        listen: SocketAddr,
        #[arg(long)]
        instance: String,
        #[arg(long)]
        origin: Vec<String>,
    },
    Seed {
        #[arg(long)]
        room: String,
        #[arg(long)]
        file: PathBuf,
    },
    Grant {
        #[arg(long)]
        room: String,
        #[arg(long)]
        key: String,
        #[arg(long)]
        actor: String,
        #[arg(long)]
        role: String,
        #[arg(long)]
        expires_ms: i64,
        #[arg(long)]
        owner: Option<String>,
    },
    Revoke {
        #[arg(long)]
        room: String,
        #[arg(long)]
        key: String,
    },
    Compact {
        #[arg(long)]
        room: String,
        #[arg(long)]
        key: String,
        #[arg(long)]
        epoch: u64,
    },
    Schema,
}
enum Database {
    Local(Arc<vellum_sync_sqlite::SqliteLog>),
    Cloud(Arc<vellum_sync_postgres::PostgresLog>),
}
impl Database {
    fn log(&self) -> Arc<dyn UpdateLog> {
        match self {
            Self::Local(l) => l.clone(),
            Self::Cloud(l) => l.clone(),
        }
    }
    fn access(&self) -> Arc<dyn AccessControl> {
        match self {
            Self::Local(l) => l.clone(),
            Self::Cloud(l) => l.clone(),
        }
    }
}
fn env(name: &str) -> Result<String, Error> {
    std::env::var(name)
        .ok()
        .filter(|s| !s.is_empty())
        .ok_or(Error::Unavailable)
}
fn key(name: &str) -> Result<SecretString, Error> {
    vellum_secrets_keyring::Keychain::new(vellum_types::brand::KEYCHAIN_SERVICE)
        .get(name)
        .map_err(|_| Error::Unavailable)?
        .ok_or(Error::Forbidden)
}
fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}
fn run(args: Args) -> Result<(), Error> {
    if matches!(args.command, Command::Schema) {
        println!(
            "{}",
            serde_json::json!({"client":client_schema(), "server":server_schema()})
        );
        return Ok(());
    }
    // Provision the private data directory before opening SQLite and its WAL.
    let disk = vellum_sync_disk::DiskSnapshots::open(&args.data)?;
    let database = if args.cloud {
        let products = env("VELLUM_SYNC_PRODUCTS")?
            .split(',')
            .map(str::to_owned)
            .collect();
        Database::Cloud(Arc::new(
            vellum_sync_postgres::PostgresLog::connect(&env("DATABASE_URL")?)?
                .require_cloud_sync(products)?,
        ))
    } else {
        Database::Local(Arc::new(vellum_sync_sqlite::SqliteLog::open(
            args.data.join("sync.sqlite"),
        )?))
    };
    match args.command {
        Command::Grant {
            room,
            key: name,
            actor,
            role,
            expires_ms,
            owner,
        } => {
            let room = RoomId::new(room).map_err(|_| Error::InvalidUpdate)?;
            let secret = key(&name)?;
            if secret.expose().len() < 32 || expires_ms <= now() {
                return Err(Error::InvalidUpdate);
            }
            let actor: Actor = serde_json::from_str(&actor).map_err(|_| Error::InvalidUpdate)?;
            if !matches!(actor, Actor::Person { .. } | Actor::Agent { .. }) {
                return Err(Error::InvalidUpdate);
            }
            let role = match role.as_str() {
                "editor" => Role::Editor,
                "commenter" => Role::Commenter,
                "viewer" => Role::Viewer,
                _ => return Err(Error::InvalidUpdate),
            };
            let grant = Grant { actor, role };
            match database {
                Database::Local(l) => l.grant(&secret, &room, &grant, expires_ms)?,
                Database::Cloud(l) => {
                    let owner = owner
                        .filter(|o| !o.is_empty())
                        .ok_or(Error::InvalidUpdate)?;
                    l.grant(&secret, &room, &grant, expires_ms, Some(&owner))?;
                }
            }
            println!("Grant saved");
            return Ok(());
        }
        Command::Revoke { room, key: name } => {
            let room = RoomId::new(room).map_err(|_| Error::InvalidUpdate)?;
            let secret = key(&name)?;
            match database {
                Database::Local(l) => l.revoke(&secret, &room)?,
                Database::Cloud(l) => l.revoke(&secret, &room)?,
            }
            println!("Grant revoked");
            return Ok(());
        }
        command => {
            let snapshots: Arc<dyn SnapshotStore> = if args.cloud {
                Arc::new(vellum_assets_s3::R2Snapshots::connect(
                    &env("VELLUM_SYNC_S3_ENDPOINT")?,
                    &env("VELLUM_SYNC_S3_BUCKET")?,
                    key("sync-storage-access")?,
                    key("sync-storage-secret")?,
                )?)
            } else {
                Arc::new(disk)
            };
            let service = Arc::new(SyncService::new(
                database.log(),
                snapshots,
                database.access(),
            ));
            match command {
                Command::Seed { room, file } => {
                    use std::io::Read;
                    let mut bytes = Vec::new();
                    std::fs::File::open(file)
                        .map_err(|_| Error::Unavailable)?
                        .take(MAX_SNAPSHOT_BYTES as u64 + 1)
                        .read_to_end(&mut bytes)
                        .map_err(|_| Error::Unavailable)?;
                    if bytes.len() > MAX_SNAPSHOT_BYTES {
                        return Err(Error::Limit);
                    }
                    let doc = vellum_codec_json::JsonCodec::new()
                        .decode(&bytes)
                        .map_err(|_| Error::InvalidUpdate)?;
                    service.seed(&RoomId::new(room).map_err(|_| Error::InvalidUpdate)?, &doc)?;
                    println!("Room created");
                }
                Command::Compact {
                    room,
                    key: name,
                    epoch,
                } => {
                    let epoch = service.compact(
                        &key(&name)?,
                        &RoomId::new(room).map_err(|_| Error::InvalidUpdate)?,
                        epoch,
                        now(),
                    )?;
                    println!("Compacted; sync epoch {epoch}");
                }
                Command::Serve {
                    listen,
                    instance,
                    origin,
                } => {
                    let presence: Arc<dyn Presence> = if args.cloud {
                        Arc::new(vellum_sync_redis::RedisPresence::connect(&env(
                            "VELLUM_SYNC_REDIS_URL",
                        )?)?)
                    } else {
                        Arc::new(vellum_sync_server::memory::MemoryPresence::default())
                    };
                    let server = vellum_sync_ws::Server::start(
                        listen,
                        service,
                        Some(presence),
                        instance,
                        origin,
                    )?;
                    println!("Sync listening on {}", server.address());
                    // The transport owns signal handling and graceful shutdown; no listener exists otherwise.
                    vellum_sync_ws::wait_for_shutdown()?;
                    server.stop();
                }
                _ => unreachable!(),
            }
        }
    }
    Ok(())
}
fn main() {
    if let Err(error) = run(Args::parse()) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
