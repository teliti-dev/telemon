use telemon_ext::*;

const ID: &str = "net.teliti.backup";

fn f(path: &str, json: &str) -> (String, Vec<u8>) {
    (path.to_string(), json.as_bytes().to_vec())
}

const INIT: &str = r#"{"ops":[
 {"op":"create_table","table":"x_net_teliti_backup__runs","columns":[
   {"name":"id","type":"integer","primaryKey":true,"autoIncrement":true},
   {"name":"router","type":"text","notNull":true},
   {"name":"ok","type":"boolean","default":false},
   {"name":"at","type":"timestamp","notNull":true},
   {"name":"note","type":"text","default":"it's fine"}]},
 {"op":"create_index","name":"x_net_teliti_backup__runs_router","table":"x_net_teliti_backup__runs","columns":["router","at"]}]}"#;

#[test]
fn namespace_and_sql_for_both_backends() {
    assert_eq!(namespace(ID), "x_net_teliti_backup__");
    let m = parse_migrations(ID, &[f("migrations/0001-init.json", INIT)]).unwrap();
    assert_eq!((m[0].version, m[0].name.as_str()), (1, "init"));
    let sqlite = migration_sql(&m[0], Dialect::Sqlite);
    assert_eq!(sqlite[0], "CREATE TABLE x_net_teliti_backup__runs (id INTEGER PRIMARY KEY, router TEXT NOT NULL, ok INTEGER DEFAULT 0, at INTEGER NOT NULL, note TEXT DEFAULT 'it''s fine')");
    assert_eq!(sqlite[1], "CREATE INDEX x_net_teliti_backup__runs_router ON x_net_teliti_backup__runs (router, at)");
    let pg = migration_sql(&m[0], Dialect::Postgres);
    assert_eq!(pg[0], "CREATE TABLE x_net_teliti_backup__runs (id BIGSERIAL PRIMARY KEY, router TEXT NOT NULL, ok BOOLEAN DEFAULT false, at BIGINT NOT NULL, note TEXT DEFAULT 'it''s fine')");
}

#[test]
fn names_outside_the_namespace_and_unsafe_shapes_are_refused() {
    let t = |ops: &str| parse_migrations(ID, &[f("migrations/0001-x.json", &format!(r#"{{"ops":[{ops}]}}"#))]);
    let col = r#"{"name":"id","type":"integer"}"#;
    for (why, ops) in [
        ("tabel inti", format!(r#"{{"op":"create_table","table":"accounts","columns":[{col}]}}"#)),
        ("tabel extension lain", format!(r#"{{"op":"create_table","table":"x_net_lain__t","columns":[{col}]}}"#)),
        ("hanya awalan", format!(r#"{{"op":"create_table","table":"x_net_teliti_backup__","columns":[{col}]}}"#)),
        ("suntik lewat nama", format!(r#"{{"op":"create_table","table":"x_net_teliti_backup__t; DROP TABLE accounts","columns":[{col}]}}"#)),
        ("huruf besar", format!(r#"{{"op":"create_table","table":"x_net_teliti_backup__T","columns":[{col}]}}"#)),
        ("kolom berbahaya", r#"{"op":"create_table","table":"x_net_teliti_backup__t","columns":[{"name":"a b","type":"text"}]}"#.to_string()),
        ("referensi ke tabel inti", r#"{"op":"create_table","table":"x_net_teliti_backup__t","columns":[{"name":"a","type":"text","references":{"table":"accounts","column":"id"}}]}"#.to_string()),
        ("indeks di tabel inti", r#"{"op":"create_index","name":"x_net_teliti_backup__i","table":"events","columns":["id"]}"#.to_string()),
        ("drop tabel inti", r#"{"op":"drop_table","table":"events"}"#.to_string()),
        ("tanpa kolom", r#"{"op":"create_table","table":"x_net_teliti_backup__t","columns":[]}"#.to_string()),
        ("kolom ganda", format!(r#"{{"op":"create_table","table":"x_net_teliti_backup__t","columns":[{col},{col}]}}"#)),
        ("add_column notNull tanpa bawaan", r#"{"op":"add_column","table":"x_net_teliti_backup__t","column":{"name":"a","type":"text","notNull":true}}"#.to_string()),
        ("add_column unik", r#"{"op":"add_column","table":"x_net_teliti_backup__t","column":{"name":"a","type":"text","unique":true}}"#.to_string()),
        ("autoIncrement bukan kunci", r#"{"op":"create_table","table":"x_net_teliti_backup__t","columns":[{"name":"a","type":"text","autoIncrement":true}]}"#.to_string()),
        ("bawaan objek", r#"{"op":"create_table","table":"x_net_teliti_backup__t","columns":[{"name":"a","type":"text","default":{"x":1}}]}"#.to_string()),
        ("operasi tak dikenal", r#"{"op":"raw_sql","sql":"DROP TABLE x"}"#.to_string()),
        ("kolom tak dikenal", r#"{"op":"drop_table","table":"x_net_teliti_backup__t","cascade":true}"#.to_string()),
    ] {
        assert!(t(&ops).is_err(), "{why} harus ditolak");
    }
    assert!(t(r#"{"op":"drop_table","table":"x_net_teliti_backup__t"}"#).is_ok());
    // Nama berkas, urutan, dan bentuk berkas.
    assert!(parse_migrations(ID, &[f("migrations/init.json", INIT)]).is_err());
    assert!(parse_migrations(ID, &[f("migrations/0002-b.json", INIT)]).is_err(), "harus mulai dari 1");
    assert!(parse_migrations(ID, &[f("migrations/0001-a.json", INIT), f("migrations/0001-b.json", INIT)]).is_err(), "versi ganda");
    assert!(parse_migrations(ID, &[f("migrations/0001-a.json", r#"{"ops":[]}"#)]).is_err());
    assert!(parse_migrations(ID, &[f("migrations/0001-a.json", "bukan json")]).is_err());
    assert!(parse_migrations(ID, &[]).unwrap().is_empty(), "tanpa migrasi itu sah");
}

#[test]
fn plan_applies_only_pending_and_guards_edits_and_downgrades() {
    let add = r#"{"ops":[{"op":"add_column","table":"x_net_teliti_backup__runs","column":{"name":"size","type":"integer","default":0}}]}"#;
    let pkg = parse_migrations(ID, &[f("migrations/0001-init.json", INIT), f("migrations/0002-size.json", add)]).unwrap();
    assert_eq!(plan_migrations(&[], &pkg).unwrap().len(), 2);
    let applied = vec![(1, pkg[0].checksum.clone())];
    let p = plan_migrations(&applied, &pkg).unwrap();
    assert_eq!(p.iter().map(|m| m.version).collect::<Vec<_>>(), vec![2]);
    assert!(plan_migrations(&[(1, pkg[0].checksum.clone()), (2, pkg[1].checksum.clone())], &pkg).unwrap().is_empty());
    // Berkas yang sudah terapan diedit.
    assert!(plan_migrations(&[(1, "salah".into())], &pkg).unwrap_err().contains("berubah"));
    // Paket lebih lama daripada data.
    let old = parse_migrations(ID, &[f("migrations/0001-init.json", INIT)]).unwrap();
    assert!(plan_migrations(&[(1, old[0].checksum.clone()), (2, "x".into())], &old).unwrap_err().contains("lebih baru"));
}

#[test]
fn package_with_bad_migrations_is_rejected_at_open() {
    let manifest = format!(r#"{{"id":"{ID}","name":"B","version":"1.0.0","apiVersion":1,"publisher":"T"}}"#);
    let good = vec![("manifest.json".to_string(), manifest.clone().into_bytes()), ("migrations/0001-init.json".to_string(), INIT.as_bytes().to_vec())];
    let keys = TrustedKeys { allow_unsigned: true, ..Default::default() };
    assert!(Package::open(&pack(&good, None).unwrap(), &keys, &Limits::default()).is_ok());
    let bad = vec![("manifest.json".to_string(), manifest.into_bytes()), ("migrations/0001-init.json".to_string(), br#"{"ops":[{"op":"drop_table","table":"events"}]}"#.to_vec())];
    assert!(matches!(Package::open(&pack(&bad, None).unwrap(), &keys, &Limits::default()), Err(PackageError::Manifest(_))));
}
