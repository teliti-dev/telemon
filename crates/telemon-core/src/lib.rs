//! Inti Telemon: model PPP dan klien RouterOS API. Tanpa penyimpanan dan tanpa UI.
//!
//! Klien memakai `mikrotik-rs` (AGPL-3.0, ADR 0011) di balik trait [`RouterApi`] agar bisa diganti.

pub mod model;
pub mod router;

pub use model::{PppActive, PppProfile, PppSecret};
pub use router::{Error, MikrotikClient, Row, RouterApi, ppp_active, ppp_profiles, ppp_secrets};
