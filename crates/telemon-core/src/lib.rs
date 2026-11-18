//! Inti Telemon: model PPP dan klien RouterOS API. Tanpa penyimpanan dan tanpa UI.
//!
//! Klien memakai `mikrotik-rs` (AGPL-3.0, ADR 0011) di balik trait [`RouterApi`] agar bisa diganti.

pub mod events;
pub mod model;
pub mod poller;
pub mod router;
pub mod store;

pub use model::{PppActive, PppProfile, PppSecret};
pub use router::{Error, MikrotikClient, Row, RouterApi, ppp_active, ppp_profiles, ppp_secrets};
pub use events::{Detector, Event, EventKind, LogLine};
pub use poller::{poll_once, run_poller, user_status, UserStatus};
pub use store::{EventQuery, PurgeCounts, RetentionPolicy, Store, StoredEvent, SyncReport, UserMeta, UserRow};
