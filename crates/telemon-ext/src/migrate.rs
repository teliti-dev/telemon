//! Migrasi database extension (ADR 0028): format deklaratif di paket (`migrations/NNNN-nama.json`),
//! divalidasi ketat dan diterjemahkan ke SQL untuk SQLite atau PostgreSQL. Extension tidak pernah
//! membuka database; hanya core yang menjalankan hasilnya, dalam transaksi.
//!
//! Semua nama tabel dan indeks wajib diawali `x_<id>__` (id dengan titik menjadi garis bawah).

use serde::Deserialize;
use sha2::{Digest, Sha256};

pub const MAX_MIGRATIONS: usize = 100;
const MAX_OPS: usize = 50;
const MAX_COLUMNS: usize = 60;

/// Awalan nama untuk satu extension: `net.teliti.backup` → `x_net_teliti_backup__`.
pub fn namespace(ext_id: &str) -> String {
    format!("x_{}__", ext_id.replace(['.', '-'], "_"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    Sqlite,
    Postgres,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ColType {
    Integer,
    Text,
    Real,
    Boolean,
    Blob,
    /// Milidetik sejak epoch (bilangan bulat).
    Timestamp,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Column {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: ColType,
    #[serde(default)]
    pub primary_key: bool,
    #[serde(default)]
    pub auto_increment: bool,
    #[serde(default)]
    pub not_null: bool,
    #[serde(default)]
    pub unique: bool,
    #[serde(default)]
    pub default: Option<serde_json::Value>,
    #[serde(default)]
    pub references: Option<Reference>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reference {
    pub table: String,
    pub column: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Op {
    CreateTable { table: String, columns: Vec<Column> },
    AddColumn { table: String, column: Column },
    CreateIndex { name: String, table: String, columns: Vec<String>, #[serde(default)] unique: bool },
    DropTable { table: String },
    DropIndex { name: String },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    ops: Vec<Op>,
}

#[derive(Debug, Clone)]
pub struct Migration {
    pub version: u32,
    pub name: String,
    /// SHA-256 (heksa) isi berkas persis; migrasi yang sudah terapan tidak boleh berubah.
    pub checksum: String,
    pub ops: Vec<Op>,
}

fn ident(s: &str, max: usize) -> bool {
    let mut c = s.chars();
    c.next().is_some_and(|f| f.is_ascii_lowercase()) && s.len() <= max && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

fn owned(ns: &str, name: &str) -> bool {
    name.starts_with(ns) && name.len() > ns.len() && name.len() <= 63 && ident(name, 63)
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Nama berkas `migrations/0001-nama.json` → (versi, nama).
fn parse_name(path: &str) -> Option<(u32, String)> {
    let file = path.strip_prefix("migrations/")?.strip_suffix(".json")?;
    let (num, name) = file.split_once('-')?;
    if num.is_empty() || num.len() > 6 || !num.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let ok = !name.is_empty() && name.len() <= 40 && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
    if !ok {
        return None;
    }
    Some((num.parse().ok()?, name.to_string()))
}

/// Baca semua `migrations/*` dari berkas paket. Versi harus berurutan mulai 1 tanpa lubang.
pub fn parse_all(ext_id: &str, files: &[(String, Vec<u8>)]) -> Result<Vec<Migration>, String> {
    let ns = namespace(ext_id);
    let mut out: Vec<Migration> = Vec::new();
    for (path, bytes) in files.iter().filter(|(p, _)| p.starts_with("migrations/")) {
        let (version, name) = parse_name(path).ok_or_else(|| format!("Nama berkas migrasi tidak valid: {path} (pakai migrations/0001-nama.json)"))?;
        let f: File = serde_json::from_slice(bytes).map_err(|e| format!("{path}: {e}"))?;
        if f.ops.is_empty() || f.ops.len() > MAX_OPS {
            return Err(format!("{path}: jumlah operasi harus 1-{MAX_OPS}"));
        }
        for op in &f.ops {
            validate(&ns, op).map_err(|e| format!("{path}: {e}"))?;
        }
        out.push(Migration { version, name, checksum: hex(&Sha256::digest(bytes)), ops: f.ops });
    }
    if out.len() > MAX_MIGRATIONS {
        return Err(format!("Maksimal {MAX_MIGRATIONS} migrasi"));
    }
    out.sort_by_key(|m| m.version);
    for (i, m) in out.iter().enumerate() {
        if m.version != i as u32 + 1 {
            return Err(format!("Versi migrasi harus berurutan mulai 1 tanpa lubang atau ganda (ditemukan {})", m.version));
        }
    }
    Ok(out)
}

fn validate_column(ns: &str, c: &Column) -> Result<(), String> {
    if !ident(&c.name, 40) {
        return Err(format!("Nama kolom tidak valid: {}", c.name));
    }
    if c.auto_increment && !(c.primary_key && c.ty == ColType::Integer) {
        return Err(format!("{}: autoIncrement hanya untuk kunci utama integer", c.name));
    }
    match &c.default {
        None | Some(serde_json::Value::Null) => {}
        Some(serde_json::Value::Number(_)) | Some(serde_json::Value::Bool(_)) => {}
        Some(serde_json::Value::String(s)) => {
            if s.len() > 200 || s.contains('\0') {
                return Err(format!("{}: nilai bawaan terlalu panjang", c.name));
            }
        }
        Some(_) => return Err(format!("{}: nilai bawaan harus angka, boolean, atau teks", c.name)),
    }
    if let Some(r) = &c.references {
        if !owned(ns, &r.table) || !ident(&r.column, 40) {
            return Err(format!("{}: referensi hanya ke tabel milik extension ({ns}...)", c.name));
        }
    }
    Ok(())
}

fn validate(ns: &str, op: &Op) -> Result<(), String> {
    let bad_table = |t: &str| format!("Nama tabel harus diawali {ns} dan hanya huruf kecil, angka, garis bawah: {t}");
    match op {
        Op::CreateTable { table, columns } => {
            if !owned(ns, table) {
                return Err(bad_table(table));
            }
            if columns.is_empty() || columns.len() > MAX_COLUMNS {
                return Err(format!("{table}: jumlah kolom harus 1-{MAX_COLUMNS}"));
            }
            let mut names = std::collections::HashSet::new();
            for c in columns {
                validate_column(ns, c)?;
                if !names.insert(&c.name) {
                    return Err(format!("{table}: kolom ganda {}", c.name));
                }
            }
            if columns.iter().filter(|c| c.primary_key).count() > 1 {
                return Err(format!("{table}: hanya satu kunci utama (kunci majemuk belum didukung)"));
            }
        }
        Op::AddColumn { table, column } => {
            if !owned(ns, table) {
                return Err(bad_table(table));
            }
            validate_column(ns, column)?;
            if column.primary_key || column.unique {
                return Err(format!("{}: add_column tidak boleh kunci utama atau unik", column.name));
            }
            if column.not_null && matches!(column.default, None | Some(serde_json::Value::Null)) {
                return Err(format!("{}: kolom notNull yang ditambahkan butuh nilai bawaan", column.name));
            }
        }
        Op::CreateIndex { name, table, columns, .. } => {
            if !owned(ns, name) || !owned(ns, table) {
                return Err(bad_table(if owned(ns, name) { table } else { name }));
            }
            if columns.is_empty() || columns.len() > 8 || !columns.iter().all(|c| ident(c, 40)) {
                return Err(format!("{name}: kolom indeks tidak valid"));
            }
        }
        Op::DropTable { table } => {
            if !owned(ns, table) {
                return Err(bad_table(table));
            }
        }
        Op::DropIndex { name } => {
            if !owned(ns, name) {
                return Err(bad_table(name));
            }
        }
    }
    Ok(())
}

fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

fn column_sql(c: &Column, d: Dialect) -> String {
    let ty = match (c.ty, d) {
        (ColType::Integer, Dialect::Sqlite) | (ColType::Boolean, Dialect::Sqlite) | (ColType::Timestamp, Dialect::Sqlite) => "INTEGER",
        (ColType::Integer, Dialect::Postgres) | (ColType::Timestamp, Dialect::Postgres) => "BIGINT",
        (ColType::Boolean, Dialect::Postgres) => "BOOLEAN",
        (ColType::Text, _) => "TEXT",
        (ColType::Real, Dialect::Sqlite) => "REAL",
        (ColType::Real, Dialect::Postgres) => "DOUBLE PRECISION",
        (ColType::Blob, Dialect::Sqlite) => "BLOB",
        (ColType::Blob, Dialect::Postgres) => "BYTEA",
    };
    let mut s = if c.auto_increment && d == Dialect::Postgres { format!("{} BIGSERIAL", c.name) } else { format!("{} {ty}", c.name) };
    if c.primary_key {
        s.push_str(" PRIMARY KEY");
    }
    if c.not_null && !c.primary_key {
        s.push_str(" NOT NULL");
    }
    if c.unique {
        s.push_str(" UNIQUE");
    }
    match &c.default {
        Some(serde_json::Value::Number(n)) => s.push_str(&format!(" DEFAULT {n}")),
        Some(serde_json::Value::Bool(b)) => match d {
            Dialect::Sqlite => s.push_str(&format!(" DEFAULT {}", *b as i32)),
            Dialect::Postgres => s.push_str(&format!(" DEFAULT {b}")),
        },
        Some(serde_json::Value::String(t)) => s.push_str(&format!(" DEFAULT {}", quote(t))),
        _ => {}
    }
    if let Some(r) = &c.references {
        s.push_str(&format!(" REFERENCES {}({})", r.table, r.column));
    }
    s
}

/// SQL untuk satu migrasi (satu pernyataan per elemen). Nama sudah divalidasi saat `parse_all`.
pub fn to_sql(m: &Migration, d: Dialect) -> Vec<String> {
    m.ops
        .iter()
        .map(|op| match op {
            Op::CreateTable { table, columns } => format!("CREATE TABLE {table} ({})", columns.iter().map(|c| column_sql(c, d)).collect::<Vec<_>>().join(", ")),
            Op::AddColumn { table, column } => format!("ALTER TABLE {table} ADD COLUMN {}", column_sql(column, d)),
            Op::CreateIndex { name, table, columns, unique } => format!("CREATE {}INDEX {name} ON {table} ({})", if *unique { "UNIQUE " } else { "" }, columns.join(", ")),
            Op::DropTable { table } => format!("DROP TABLE {table}"),
            Op::DropIndex { name } => format!("DROP INDEX {name}"),
        })
        .collect()
}

/// Migrasi yang belum terapan. `applied` = (versi, checksum) yang tercatat. Gagal bila ada yang sudah
/// terapan tetapi berubah isinya, atau bila data lebih baru daripada paket (penurunan versi).
pub fn plan<'a>(applied: &[(u32, String)], pkg: &'a [Migration]) -> Result<Vec<&'a Migration>, String> {
    for (v, sum) in applied {
        match pkg.iter().find(|m| m.version == *v) {
            None => return Err(format!("Data extension sudah dimigrasi ke versi {v}, lebih baru daripada paket ini (penurunan versi tidak didukung)")),
            Some(m) if m.checksum != *sum => return Err(format!("Migrasi {v} ({}) sudah terapan tetapi isinya berubah; migrasi yang terapan tidak boleh diedit", m.name)),
            Some(_) => {}
        }
    }
    Ok(pkg.iter().filter(|m| !applied.iter().any(|(v, _)| *v == m.version)).collect())
}
