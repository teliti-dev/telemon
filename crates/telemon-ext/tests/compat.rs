use telemon_ext::*;

fn m(id: &str, version: &str, telemon: Option<&str>, requires: &[(&str, &str, bool)]) -> Manifest {
    let req: Vec<String> = requires.iter().map(|(i, v, o)| format!(r#"{{"id":"{i}","version":"{v}","optional":{o}}}"#)).collect();
    let tm = telemon.map_or(String::new(), |t| format!(r#","telemon":"{t}""#));
    let j = format!(r#"{{"id":"{id}","name":"{id}","version":"{version}","apiVersion":1,"publisher":"T","requires":[{}]{tm}}}"#, req.join(","));
    let m: Manifest = serde_json::from_str(&j).unwrap();
    m.validate().unwrap();
    m
}

fn inst(l: &[(&str, &str)]) -> Vec<(String, String)> {
    l.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
}

#[test]
fn versions_and_ranges() {
    let v = |s: &str| Version::parse(s).unwrap();
    assert_eq!(v("0.1.0-rc.6"), v("0.1.0"), "praterbit diabaikan");
    assert!(Version::parse("1.2").is_none() && Version::parse("a.b.c").is_none() && Version::parse("1.2.3.4").is_none());
    let r = |s: &str| Range::parse(s).unwrap();
    assert!(r("^1.2.3").matches(v("1.9.0")) && !r("^1.2.3").matches(v("2.0.0")) && !r("^1.2.3").matches(v("1.2.2")));
    assert!(r("^0.2.1").matches(v("0.2.9")) && !r("^0.2.1").matches(v("0.3.0")), "0.x: minor mengunci");
    assert!(r("^0.0.3").matches(v("0.0.3")) && !r("^0.0.3").matches(v("0.0.4")));
    assert!(r("~1.2.3").matches(v("1.2.9")) && !r("~1.2.3").matches(v("1.3.0")));
    assert!(r(">=1.0.0 <2.0.0").matches(v("1.5.0")) && !r(">=1.0.0 <2.0.0").matches(v("2.0.0")));
    assert!(r("=1.0.0").matches(v("1.0.0")) && r("1.0.0").matches(v("1.0.0")) && !r("1.0.0").matches(v("1.0.1")));
    assert!(r("*").matches(v("9.9.9")));
    for bad in ["", "^x", ">=", "1.x", "^1.2.3 junk"] {
        assert!(Range::parse(bad).is_err(), "{bad}");
    }
}

#[test]
fn telemon_version_range_is_checked() {
    let a = m("net.t.a", "1.0.0", Some(">=0.2.0 <0.4.0"), &[]);
    assert!(check_install(&a, "0.3.1", &[], &[]).is_empty());
    assert!(check_install(&a, "0.3.0-rc.1", &[], &[]).is_empty(), "rc dihitung sebagai 0.3.0");
    assert_eq!(check_install(&a, "0.4.0", &[], &[]), vec![Problem::TelemonIncompatible { needs: ">=0.2.0 <0.4.0".into(), have: "0.4.0".into() }]);
    assert!(check_install(&m("net.t.b", "1.0.0", None, &[]), "9.0.0", &[], &[]).is_empty(), "tanpa rentang = tidak dibatasi");
}

#[test]
fn dependencies_missing_incompatible_and_optional() {
    let a = m("net.t.a", "1.0.0", None, &[("net.t.b", "^1.2.0", false), ("net.t.c", "^2.0.0", true)]);
    assert_eq!(check_install(&a, "0.1.0", &[], &[]), vec![Problem::MissingDependency { id: "net.t.b".into(), needs: "^1.2.0".into() }], "opsional yang hilang tidak menghalangi");
    assert!(check_install(&a, "0.1.0", &inst(&[("net.t.b", "1.4.0")]), &[]).is_empty());
    assert_eq!(
        check_install(&a, "0.1.0", &inst(&[("net.t.b", "1.1.0")]), &[]),
        vec![Problem::IncompatibleDependency { id: "net.t.b".into(), needs: "^1.2.0".into(), have: "1.1.0".into() }]
    );
    // Opsional yang terpasang dengan versi salah tetap masalah.
    let p = check_install(&a, "0.1.0", &inst(&[("net.t.b", "1.2.0"), ("net.t.c", "1.0.0")]), &[]);
    assert_eq!(p, vec![Problem::IncompatibleDependency { id: "net.t.c".into(), needs: "^2.0.0".into(), have: "1.0.0".into() }]);
    let st = dep_status(&a, &inst(&[("net.t.b", "1.2.0")]));
    assert_eq!((st[0].state, st[1].state, st[1].optional), ("ok", "missing", true));
}

#[test]
fn updating_a_dependency_must_not_break_its_dependents() {
    let a = m("net.t.a", "1.0.0", None, &[("net.t.b", "^1.0.0", false)]);
    let b2 = m("net.t.b", "2.0.0", None, &[]);
    let p = check_install(&b2, "0.1.0", &[], std::slice::from_ref(&a));
    assert_eq!(p, vec![Problem::BreaksDependent { id: "net.t.a".into(), needs: "^1.0.0".into() }]);
    let b12 = m("net.t.b", "1.2.0", None, &[]);
    assert!(check_install(&b12, "0.1.0", &[], &[a.clone()]).is_empty());
    assert_eq!(dependents_of("net.t.b", &[a.clone(), b12.clone()]).len(), 1);
    // Opsional tidak dihitung sebagai yang bergantung wajib.
    let opt = m("net.t.o", "1.0.0", None, &[("net.t.b", "^1.0.0", true)]);
    assert!(dependents_of("net.t.b", &[opt]).is_empty());
}

#[test]
fn cycles_are_detected() {
    let a = m("net.t.a", "1.0.0", None, &[("net.t.b", "*", false)]);
    let b = m("net.t.b", "1.0.0", None, &[("net.t.a", "*", false)]);
    assert!(check_install(&a, "0.1.0", &inst(&[("net.t.b", "1.0.0")]), &[b.clone()]).contains(&Problem::Cycle { id: "net.t.a".into() }));
    // Self-require ditolak oleh manifest.
    let j = r#"{"id":"net.t.s","name":"s","version":"1.0.0","apiVersion":1,"publisher":"T","requires":[{"id":"net.t.s","version":"*"}]}"#;
    assert!(serde_json::from_str::<Manifest>(j).unwrap().validate().is_err());
}

#[test]
fn install_order_puts_dependencies_first_and_skips_installed() {
    let a = m("net.t.a", "1.0.0", None, &[("net.t.b", "^1.0.0", false), ("net.t.d", "^1.0.0", false)]);
    let store = vec![m("net.t.b", "1.3.0", None, &[("net.t.c", "^1.0.0", false)]), m("net.t.c", "1.0.0", None, &[]), m("net.t.d", "1.0.0", None, &[])];
    let lookup = |id: &str| store.iter().find(|x| x.id == id).cloned();
    let order = install_order(&a, &[], &lookup).unwrap();
    assert_eq!(order, vec!["net.t.c", "net.t.b", "net.t.d", "net.t.a"]);
    // Yang sudah terpasang dan cocok tidak dipasang lagi.
    let order = install_order(&a, &inst(&[("net.t.b", "1.1.0"), ("net.t.c", "1.0.0")]), &lookup).unwrap();
    assert_eq!(order, vec!["net.t.d", "net.t.a"]);
    // Terpasang tetapi tidak cocok: galat (tidak memperbarui diam-diam).
    assert!(matches!(install_order(&a, &inst(&[("net.t.b", "2.0.0")]), &lookup), Err(Problem::IncompatibleDependency { .. })));
    // Tidak tersedia, versi toko tak cocok, melingkar.
    assert!(matches!(install_order(&a, &[], &|_| None), Err(Problem::MissingDependency { .. })));
    let old = vec![m("net.t.b", "0.9.0", None, &[]), m("net.t.d", "1.0.0", None, &[])];
    assert!(matches!(install_order(&a, &[], &|id| old.iter().find(|x| x.id == id).cloned()), Err(Problem::IncompatibleDependency { .. })));
    let a2 = m("net.t.a", "1.0.0", None, &[("net.t.b", "*", false)]);
    let cyc = vec![m("net.t.b", "1.0.0", None, &[("net.t.a", "*", false)]), a2.clone()];
    assert!(matches!(install_order(&a2, &[], &|id| cyc.iter().find(|x| x.id == id).cloned()), Err(Problem::Cycle { .. })));
}

#[test]
fn host_compatibility_is_rechecked_at_load() {
    let a = m("net.t.a", "1.0.0", Some(">=0.2.0 <0.4.0"), &[]);
    assert!(host_compatible(&a, "0.3.9") && host_compatible(&a, "0.2.0-rc.1"));
    assert!(!host_compatible(&a, "0.4.0") && !host_compatible(&a, "0.1.9"), "Telemon naik/turun di luar rentang");
    assert!(host_compatible(&m("net.t.b", "1.0.0", None, &[]), "7.0.0"));
}
