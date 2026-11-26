// Kunci official ditanam saat build dari TELEMON_OFFICIAL_EXT_KEYS; ubah nilainya harus membangun ulang crate ini
// (tanpa ini cache build menahan nilai lama).
fn main() {
    println!("cargo:rerun-if-env-changed=TELEMON_OFFICIAL_EXT_KEYS");
}
