use telemon_ext::*;

const SEC: &str = "0101010101010101010101010101010101010101010101010101010101010101";
const OTHER: &str = "0202020202020202020202020202020202020202020202020202020202020202";

fn entry(id: &str, version: &str, telemon: Option<&str>, requires: &[(&str, &str)]) -> CatalogEntry {
    let req: Vec<String> = requires.iter().map(|(i, v)| format!(r#"{{"id":"{i}","version":"{v}"}}"#)).collect();
    let tm = telemon.map_or(String::new(), |t| format!(r#","telemon":"{t}""#));
    let j = format!(r#"{{"id":"{id}","name":"{id}","version":"{version}","apiVersion":1,"publisher":"Teliti","requires":[{}]{tm}}}"#, req.join(","));
    let m: Manifest = serde_json::from_str(&j).unwrap();
    CatalogEntry { manifest: m, file: format!("{id}-{version}.tmx"), sha256: "ab".repeat(32), size: 100 }
}

fn signed(entries: Vec<CatalogEntry>, key: &str) -> (Vec<u8>, String) {
    let c = Catalog { version: 1, generated: "2026-11-26".into(), extensions: entries };
    let bytes = serde_json::to_vec(&c).unwrap();
    let sig = sign_bytes(key, &bytes).unwrap();
    (bytes, sig)
}

fn official() -> Vec<String> {
    vec![public_key_hex(SEC).unwrap()]
}

#[test]
fn signature_is_required_and_tampering_is_detected() {
    let (bytes, sig) = signed(vec![entry("net.t.a", "1.0.0", None, &[])], SEC);
    assert_eq!(open_catalog(&bytes, &sig, &official()).unwrap().extensions.len(), 1);
    let (b2, s2) = signed(vec![entry("net.t.a", "1.0.0", None, &[])], OTHER);
    assert!(open_catalog(&b2, &s2, &official()).is_err(), "kunci bukan official");
    let mut t = bytes.clone();
    t[10] ^= 1;
    assert!(open_catalog(&t, &sig, &official()).is_err(), "isi diubah");
    assert!(open_catalog(&bytes, "bukan-heksa", &official()).is_err());
    assert!(open_catalog(&bytes, &sig, &[]).is_err(), "tanpa kunci official");
    assert!(verify_bytes(&official()[0], b"x", &sign_bytes(SEC, b"x").unwrap()) && !verify_bytes(&official()[0], b"y", &sign_bytes(SEC, b"x").unwrap()));
}

#[test]
fn entries_are_validated() {
    let mut bad_file = entry("net.t.a", "1.0.0", None, &[]);
    bad_file.file = "../evil.tmx".into();
    let mut bad_sum = entry("net.t.b", "1.0.0", None, &[]);
    bad_sum.sha256 = "xyz".into();
    let mut big = entry("net.t.c", "1.0.0", None, &[]);
    big.size = 1 << 30;
    for (why, e) in [("jalur", bad_file), ("sha", bad_sum), ("ukuran", big)] {
        let (b, s) = signed(vec![e], SEC);
        assert!(open_catalog(&b, &s, &official()).is_err(), "{why}");
    }
    let (b, s) = signed(vec![entry("net.t.a", "1.0.0", None, &[]), entry("net.t.a", "1.1.0", None, &[])], SEC);
    assert!(open_catalog(&b, &s, &official()).unwrap_err().contains("ganda"));
}

#[test]
fn plan_orders_dependencies_and_respects_host_and_installed() {
    let (b, s) = signed(
        vec![
            entry("net.t.a", "1.0.0", Some(">=0.2.0"), &[("net.t.b", "^1.0.0")]),
            entry("net.t.b", "1.2.0", None, &[("net.t.c", "^1.0.0")]),
            entry("net.t.c", "1.0.0", None, &[]),
            entry("net.t.old", "1.0.0", Some("<0.2.0"), &[]),
            entry("net.t.needs-old", "1.0.0", None, &[("net.t.old", "*")]),
        ],
        SEC,
    );
    let cat = open_catalog(&b, &s, &official()).unwrap();
    let ids = |p: Vec<&CatalogEntry>| p.iter().map(|e| e.manifest.id.clone()).collect::<Vec<_>>();
    assert_eq!(ids(cat.plan("net.t.a", "0.3.0", &[]).unwrap()), vec!["net.t.c", "net.t.b", "net.t.a"]);
    let have = vec![("net.t.b".to_string(), "1.1.0".to_string()), ("net.t.c".to_string(), "1.0.0".to_string())];
    assert_eq!(ids(cat.plan("net.t.a", "0.3.0", &have).unwrap()), vec!["net.t.a"], "yang terpasang dan cocok dilewati");
    assert!(matches!(cat.plan("net.t.a", "0.1.0", &[]), Err(Problem::TelemonIncompatible { .. })), "Telemon terlalu lama");
    assert!(matches!(cat.plan("net.t.needs-old", "0.3.0", &[]), Err(Problem::MissingDependency { .. })), "dependensi tak cocok dengan Telemon ini");
    let wrong = vec![("net.t.b".to_string(), "2.0.0".to_string())];
    assert!(matches!(cat.plan("net.t.a", "0.3.0", &wrong), Err(Problem::IncompatibleDependency { .. })), "tidak memperbarui diam-diam");
    assert!(matches!(cat.plan("net.t.tidak-ada", "0.3.0", &[]), Err(Problem::MissingDependency { .. })));
}

#[test]
fn downloads_are_checked_against_the_catalog() {
    let bytes = b"isi paket".to_vec();
    let mut e = entry("net.t.a", "1.0.0", None, &[]);
    e.sha256 = sha256_hex(&bytes);
    e.size = bytes.len() as u64;
    assert!(e.check_download(&bytes).is_ok());
    assert!(e.check_download(b"isi paket!").is_err(), "ukuran");
    let mut tampered = bytes.clone();
    tampered[0] ^= 1;
    assert!(e.check_download(&tampered).is_err(), "sha256");
}
