#!/usr/bin/env python3
"""Buat manifest pembaruan Tauri (`latest.json`) dari berkas rilis yang sudah ditandatangani.

Gagal keras bila platform mana pun kehilangan berkas atau tanda tangannya: manifest setengah jadi tidak boleh
terbit, karena klien akan mengira pembaruan tersedia lalu gagal memasang.
"""
import argparse
import datetime
import json
import re
import sys
from pathlib import Path

PLATFORMS = {
    # kunci manifest -> pola berkas yang diperbarui (ada .sig di sebelahnya)
    "linux-x86_64": re.compile(r"^Telemon_.+_amd64\.AppImage$"),
    "windows-x86_64": re.compile(r"^Telemon_.+_x64-setup\.exe$"),
    # macOS tidak dibangun lagi; tambahkan kembali "darwin-*" di sini bila job macOS dihidupkan lagi.
}
SEMVER = re.compile(r"^v?[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.]+)?$")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--dir", required=True)
    ap.add_argument("--tag", required=True)
    ap.add_argument("--repo", required=True)
    ap.add_argument("--notes-file")
    ap.add_argument("--out", required=True)
    a = ap.parse_args()
    if not SEMVER.match(a.tag):
        print(f"tag bukan semver: {a.tag}", file=sys.stderr)
        return 2
    d = Path(a.dir)
    files = {p.name: p for p in d.iterdir() if p.is_file()}
    platforms = {}
    for key, pat in PLATFORMS.items():
        found = [n for n in files if pat.match(n)]
        if len(found) != 1:
            print(f"{key}: harus tepat satu berkas, ditemukan {found}", file=sys.stderr)
            return 1
        name = found[0]
        sig = files.get(name + ".sig")
        if sig is None or not sig.read_text().strip():
            print(f"{key}: tanda tangan {name}.sig tidak ada atau kosong", file=sys.stderr)
            return 1
        platforms[key] = {
            "signature": sig.read_text().strip(),
            "url": f"https://github.com/{a.repo}/releases/download/{a.tag}/{name}",
        }
    notes = Path(a.notes_file).read_text().strip() if a.notes_file and Path(a.notes_file).exists() else ""
    manifest = {
        "version": a.tag.lstrip("v"),
        "notes": notes,
        "pub_date": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "platforms": platforms,
    }
    Path(a.out).write_text(json.dumps(manifest, indent=2, ensure_ascii=False) + "\n")
    print(f"latest.json: versi {manifest['version']}, platform {', '.join(platforms)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
