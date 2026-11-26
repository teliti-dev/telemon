#!/usr/bin/env python3
"""Build src-tauri/installer/license.txt: the text of the license page in the installer.

It is a short notice (what Telemon is, the no-warranty position, extensions) followed by the full LICENSE (AGPL-3.0).
The file is committed so a local `tauri build` works; CI runs `--check` so it cannot drift from LICENSE.

  python3 scripts/make-installer-license.py          write the file
  python3 scripts/make-installer-license.py --check  fail if the committed file is out of date
"""
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "src-tauri" / "installer" / "license.txt"

NOTICE = """TELEMON
Copyright (c) 2026 Teliti (https://teliti.net/)

Telemon monitors PPPoE users and events across MikroTik routers.

LICENSE
Telemon is free software, licensed under the GNU Affero General Public License v3.0 (AGPL-3.0), whose full text follows.
You do not have to accept the AGPL in order to run Telemon; it applies when you modify or share it. If you modify Telemon
and let other people use it over a network, you must offer them the source of your version.
Source code: https://github.com/teliti-dev/telemon

NO WARRANTY, NO LIABILITY
Telemon is provided "as is", without warranty of any kind. To the extent the law allows, Teliti, the authors and the
contributors are not liable for any damage from using it, such as lost, deleted or exposed data, a security breach, a
router misconfiguration or downtime. Telemon can change your routers: read the review dialog, keep your own backups and
secure access. The full disclaimer is DISCLAIMER.md in the source repository.

EXTENSIONS
Extensions are made by their publishers, and each has its own license. Telemon does not sell, verify or enforce them.

================================================================================
"""


def build() -> str:
    text = NOTICE + "\n" + (ROOT / "LICENSE").read_text(encoding="utf-8")
    # The installer's license page is a Windows control: use CRLF line endings.
    return text.replace("\r\n", "\n").replace("\n", "\r\n")


def main() -> int:
    want = build()
    if "--check" in sys.argv:
        have = OUT.read_bytes().decode("utf-8") if OUT.exists() else ""
        if have != want:
            print(f"{OUT.relative_to(ROOT)} is out of date: run python3 scripts/make-installer-license.py", file=sys.stderr)
            return 1
        print("installer license is up to date")
        return 0
    OUT.write_bytes(want.encode("utf-8"))
    print(f"wrote {OUT.relative_to(ROOT)} ({len(want)} bytes)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
