//! Inti Telemon: model PPP dan klien RouterOS API. Tanpa penyimpanan dan tanpa UI.
//!
//! Klien memakai `mikrotik-rs` (AGPL-3.0, ADR 0011) di balik trait [`RouterApi`] agar bisa diganti.

pub mod events;
pub mod model;
pub mod monitor;
pub mod poller;
pub mod router;
pub mod service;
pub mod store;
#[cfg(feature = "tls")]
pub mod tls;
pub mod vault;
pub mod write;

pub use model::{PppActive, PppProfile, PppSecret};
pub use router::{TlsMode, Error, MikrotikClient, Row, RouterApi, ppp_active, ppp_profiles, ppp_secrets};
pub use events::{Detector, Event, EventKind, LogLine};
pub use poller::{poll_once, poll_once_status, run_poller, user_status, UserStatus};
pub use store::{NewRouter, RouterError, RouterPatch, RouterRecord, RouterRemoved, RouterStatus, normalize_fingerprint, PasswordError, AuditEntry, AuditOutcome, AuditQuery, StoredAudit, EventQuery, PurgeCounts, RetentionPolicy, Store, StoredEvent, SyncReport, UserMeta, UserRow};
pub use monitor::{MonitorConfig, SharedStore, run_purge, run_router, sync_router};
pub use write::{Change, Plan, apply, apply_audited};
pub use vault::{VaultError, generate_key};
#[cfg(feature = "tls")]
pub use tls::{Fingerprint, fetch_fingerprint, pinned_config};
pub use service::{Connector, MemorySecrets, NetConnector, Probe, Resilient, RouterEvent, RuntimeConfig, SecretStore, ServiceError, Telemon};
