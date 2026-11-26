use telemon_ext::*;

const SEC_A: &str = "0101010101010101010101010101010101010101010101010101010101010101";
const SEC_B: &str = "0202020202020202020202020202020202020202020202020202020202020202";

fn manifest() -> String {
    r#"{"id":"net.teliti.demo","name":"Demo","version":"1.0.0","apiVersion":1,"publisher":"Teliti",
        "ui":{"entry":"ui/index.js"},
        "contributes":{"widgets":[{"type":"hello","title":"Hello"}]},
        "permissions":[{"id":"data.events","required":true},{"id":"feature.ai","sensitive":true}]}"#
        .into()
}

fn files() -> Vec<(String, Vec<u8>)> {
    vec![("manifest.json".into(), manifest().into_bytes()), ("ui/index.js".into(), b"export default {}".to_vec())]
}

fn keys() -> TrustedKeys {
    TrustedKeys { official: vec![public_key_hex(SEC_A).unwrap()], publishers: vec![public_key_hex(SEC_B).unwrap()], allow_unsigned: false }
}

#[test]
fn official_signature_gives_official_tier() {
    let bytes = pack(&files(), Some(SEC_A)).unwrap();
    let p = Package::open(&bytes, &keys(), &Limits::default()).unwrap();
    assert_eq!(p.tier, Tier::Official);
    assert!(p.signed);
    assert_eq!(p.manifest.id, "net.teliti.demo");
    assert_eq!(p.file("ui/index.js").unwrap(), b"export default {}");
}

#[test]
fn trusted_publisher_gives_public_tier() {
    let bytes = pack(&files(), Some(SEC_B)).unwrap();
    let p = Package::open(&bytes, &keys(), &Limits::default()).unwrap();
    assert_eq!(p.tier, Tier::Public);
    assert!(p.signed);
}

#[test]
fn unknown_key_and_missing_signature_are_refused() {
    let other = "0303030303030303030303030303030303030303030303030303030303030303";
    let bytes = pack(&files(), Some(other)).unwrap();
    assert_eq!(Package::open(&bytes, &keys(), &Limits::default()).unwrap_err(), PackageError::Untrusted);
    let bytes = pack(&files(), None).unwrap();
    assert_eq!(Package::open(&bytes, &keys(), &Limits::default()).unwrap_err(), PackageError::Untrusted);
}

#[test]
fn allow_unsigned_is_public_and_marked_unsigned() {
    let mut k = keys();
    k.allow_unsigned = true;
    let p = Package::open(&pack(&files(), None).unwrap(), &k, &Limits::default()).unwrap();
    assert_eq!((p.tier, p.signed), (Tier::Public, false));
    // Tanda tangan valid dari kunci asing juga turun ke Public tanpa tanda "bertanda tangan".
    let other = "0303030303030303030303030303030303030303030303030303030303030303";
    let p = Package::open(&pack(&files(), Some(other)).unwrap(), &k, &Limits::default()).unwrap();
    assert_eq!((p.tier, p.signed), (Tier::Public, false));
}

#[test]
fn tampering_breaks_the_signature() {
    let mut f = files();
    let bytes = pack(&f, Some(SEC_A)).unwrap();
    // Bangun ulang dengan isi berbeda tetapi tanda tangan lama: ambil berkas tanda tangan dari paket asli.
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(&bytes)).unwrap();
    let mut sig = String::new();
    std::io::Read::read_to_string(&mut zip.by_name("signature.json").unwrap(), &mut sig).unwrap();
    f[1].1 = b"export default { evil: true }".to_vec();
    let unsigned = pack(&f, None).unwrap();
    let mut out = Vec::new();
    {
        use std::io::Write;
        let mut z = zip::ZipWriter::new(std::io::Cursor::new(&mut out));
        let o = zip::write::SimpleFileOptions::default();
        let mut src = zip::ZipArchive::new(std::io::Cursor::new(&unsigned)).unwrap();
        for i in 0..src.len() {
            let mut e = src.by_index(i).unwrap();
            let mut b = Vec::new();
            std::io::Read::read_to_end(&mut e, &mut b).unwrap();
            z.start_file(e.name().to_string(), o).unwrap();
            z.write_all(&b).unwrap();
        }
        z.start_file("signature.json", o).unwrap();
        z.write_all(sig.as_bytes()).unwrap();
        z.finish().unwrap();
    }
    assert_eq!(Package::open(&out, &keys(), &Limits::default()).unwrap_err(), PackageError::BadSignature);
}

#[test]
fn unsafe_paths_are_rejected() {
    for bad in ["../x.js", "/abs.js", "ui/../../x", "ui//x", "a\\b", "ui/./x", "c:/x"] {
        let mut f = files();
        // `pack` recusa jalur buruk; bangun zip manual untuk menguji pembaca.
        let mut out = Vec::new();
        {
            use std::io::Write;
            let mut z = zip::ZipWriter::new(std::io::Cursor::new(&mut out));
            let o = zip::write::SimpleFileOptions::default();
            for (p, b) in f.drain(..) {
                z.start_file(p, o).unwrap();
                z.write_all(&b).unwrap();
            }
            z.start_file(bad, o).unwrap();
            z.write_all(b"x").unwrap();
            z.finish().unwrap();
        }
        let mut k = keys();
        k.allow_unsigned = true;
        assert!(matches!(Package::open(&out, &k, &Limits::default()), Err(PackageError::Unsafe(_))), "{bad}");
    }
}

#[test]
fn manifest_rules() {
    let bad = |m: &str| {
        let f = vec![("manifest.json".into(), m.as_bytes().to_vec()), ("ui/index.js".into(), b"x".to_vec())];
        let mut k = keys();
        k.allow_unsigned = true;
        Package::open(&pack(&f, None).unwrap(), &k, &Limits::default()).unwrap_err()
    };
    let base = |extra: &str| format!(r#"{{"id":"net.teliti.demo","name":"D","version":"1.0.0","apiVersion":1,"publisher":"T"{extra}}}"#);
    assert!(matches!(bad(&base(r#","apiVersion":2"#).replace(r#""apiVersion":1,"#, "")), PackageError::Manifest(_)));
    assert!(matches!(bad(&base(r#","ui":{"entry":"../x.js"}"#)), PackageError::Manifest(_)));
    assert!(matches!(bad(&base(r#","ui":{"entry":"ui/missing.js"}"#)), PackageError::Manifest(_)));
    assert!(matches!(bad(&base(r#","contributes":{"widgets":[{"type":"a","title":"A"}]}"#)), PackageError::Manifest(_)), "widget butuh ui");
    assert!(matches!(bad(&base(r#","surprise":1"#)), PackageError::Manifest(_)), "kolom tak dikenal");
    assert!(matches!(bad(&base(r#","permissions":[{"id":"a"},{"id":"a"}]"#)), PackageError::Manifest(_)));
    assert!(matches!(bad(&base("").replace("net.teliti.demo", "Bad ID")), PackageError::Manifest(_)));
    assert!(matches!(bad(&base("").replace("1.0.0", "1.0")), PackageError::Manifest(_)));
}

#[test]
fn size_limits_apply() {
    let mut k = keys();
    k.allow_unsigned = true;
    let mut f = files();
    f.push(("ui/big.bin".into(), vec![0u8; 2000]));
    let bytes = pack(&f, None).unwrap();
    let lim = Limits { max_file: 1000, ..Limits::default() };
    assert!(matches!(Package::open(&bytes, &k, &lim), Err(PackageError::Limits(_))));
    let lim = Limits { max_files: 2, ..Limits::default() };
    assert!(matches!(Package::open(&bytes, &k, &lim), Err(PackageError::Limits(_))));
    let lim = Limits { max_package: 10, ..Limits::default() };
    assert_eq!(Package::open(&bytes, &k, &lim).unwrap_err(), PackageError::TooLarge);
    assert_eq!(Package::open(b"bukan zip", &k, &Limits::default()).unwrap_err(), PackageError::NotZip);
}

#[test]
fn session_declarations_require_a_sensitive_permission_and_an_update_action() {
    let t = |extra: &str| {
        let m = format!(r#"{{"id":"net.teliti.t","name":"T","version":"1.0.0","apiVersion":1,"publisher":"T","ui":{{"entry":"ui/index.js"}}{extra}}}"#);
        let f = vec![("manifest.json".into(), m.into_bytes()), ("ui/index.js".into(), b"x".to_vec())];
        let mut k = keys();
        k.allow_unsigned = true;
        Package::open(&pack(&f, None).unwrap(), &k, &Limits::default())
    };
    let good = r#","actions":[{"id":"terminal","label":"T","level":"update"}],"permissions":[{"id":"feature.router-ssh","sensitive":true}],"contributes":{"sessions":[{"id":"ssh","kind":"router-ssh","action":"terminal"}]}"#;
    assert!(t(good).is_ok());
    // Izin tidak sensitif, aksi level read, aksi tak dideklarasikan, jenis lain: semuanya ditolak.
    assert!(t(&good.replace(r#""sensitive":true"#, r#""sensitive":false"#)).is_err());
    assert!(t(&good.replace(r#""level":"update""#, r#""level":"read""#)).is_err());
    assert!(t(&good.replace(r#""action":"terminal""#, r#""action":"lain""#)).is_err());
    assert!(t(&good.replace("router-ssh\",\"action", "shell\",\"action")).is_err());
    assert!(t(&good.replace(r#"{"id":"feature.router-ssh","sensitive":true}"#, "")).is_err(), "tanpa izin sama sekali");
}
