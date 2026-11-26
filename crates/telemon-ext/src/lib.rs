//! Paket extension Telemon (ADR 0028): manifest, berkas `.tmx` (zip) dan tanda tangan Ed25519.
//!
//! Crate ini murni (tanpa penyimpanan dan tanpa jaringan): membaca bytes paket, memeriksa batas dan
//! jalur berkas, memverifikasi tanda tangan terhadap kunci tepercaya, dan menetapkan **tingkat**
//! (official atau public) dari kunci yang menandatangani. Memasang, mengaktifkan, dan izin ada di server.

mod manifest;
mod package;
mod resolve;
mod version;

pub use manifest::{Contributes, Manifest, PermissionDecl, Requirement, UiDecl, WidgetDecl, valid_id};
pub use resolve::{DepStatus, Problem, check_install, dep_status, dependents_of, host_compatible, install_order};
pub use version::{Range, Version};
pub use package::{Limits, Package, PackageError, Tier, TrustedKeys, pack, public_key_hex};

/// Versi kontrak host yang dipahami build ini. Paket dengan `apiVersion` lain ditolak.
pub const API_VERSION: u32 = 1;
