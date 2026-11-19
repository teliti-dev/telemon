#!/usr/bin/env python3
"""Samakan versi aplikasi dengan tag rilis (satu sumber: tag). Dipakai workflow rilis sebelum build.

Memperbarui src-tauri/tauri.conf.json, src-tauri/Cargo.toml ([package]) dan package.json.
"""
import json
import re
import sys
from pathlib import Path

SEMVER = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.]+)?$")


def main() -> int:
    if len(sys.argv) != 2 or not SEMVER.match(sys.argv[1]):
        print("pakai: set-version.py <semver>, mis. 0.1.0 atau 0.1.0-rc.1", file=sys.stderr)
        return 2
    version = sys.argv[1]

    for path in ("src-tauri/tauri.conf.json", "package.json"):
        p = Path(path)
        data = json.loads(p.read_text())
        data["version"] = version
        p.write_text(json.dumps(data, indent=2, ensure_ascii=False) + "\n")

    cargo = Path("src-tauri/Cargo.toml")
    text = cargo.read_text()
    # Hanya baris `version` pertama di bagian [package].
    new, n = re.subn(r'(?ms)(^\[package\].*?^version\s*=\s*")[^"]*(")', rf"\g<1>{version}\g<2>", text, count=1)
    if n != 1:
        print("version di [package] Cargo.toml tidak ditemukan", file=sys.stderr)
        return 1
    cargo.write_text(new)
    print(f"versi diset ke {version}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
