#!/usr/bin/env python3
"""Build src-tauri/installer/installer.nsi: the Tauri NSIS template with Telemon's small changes.

The stock Tauri template sets the installer's file description to the product name and has no CompanyName. Instead of keeping
a hand-edited copy of a 1,350-line template, this script takes the template that is embedded in the pinned @tauri-apps/cli
and applies the patches below. Every patch must match exactly once, so a Tauri upgrade that moves an anchor fails loudly.

  python3 scripts/make-nsis-template.py          write the file
  python3 scripts/make-nsis-template.py --check  fail if the committed file is not what this script would produce

After upgrading @tauri-apps/cli, run it without --check and review the diff of src-tauri/installer/installer.nsi.
"""
import glob
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "src-tauri" / "installer" / "installer.nsi"

START = b"Unicode true\nManifestDPIAware true"
END = b"Signing NSIS plugins"  # the next string in the binary; the template is everything before it

PATCHES = [
    # (anchor, replacement). The anchor must occur exactly once in the template.
    (
        'VIAddVersionKey "FileDescription" "${PRODUCTNAME}"\n',
        'VIAddVersionKey "FileDescription" "${PRODUCTNAME} Setup"\n'
        '; TELEMON: the stock template has no CompanyName and no Comments.\n'
        'VIAddVersionKey "CompanyName" "${MANUFACTURER}"\n'
        'VIAddVersionKey "Comments" "Monitor PPPoE users and events across MikroTik routers. Free software under the AGPL-3.0, provided without warranty. https://github.com/teliti-dev/telemon"\n',
    ),
    (
        "; 1. Welcome Page\n",
        "; 1. Welcome Page\n"
        "; TELEMON: say what Telemon is and that it comes without warranty (English only).\n"
        '!define MUI_WELCOMEPAGE_TEXT "Telemon monitors PPPoE users and events across MikroTik routers.$\\r$\\n$\\r$\\n'
        "Telemon is free software (AGPL-3.0) and is provided without warranty. It can change your routers, so read the review dialog "
        'and keep your own backups.$\\r$\\n$\\r$\\nClick Next to continue."\n',
    ),
]


def upstream() -> str:
    nodes = sorted(glob.glob(str(ROOT / "node_modules" / "@tauri-apps" / "cli-*" / "*.node")))
    if not nodes:
        sys.exit("no @tauri-apps/cli binary found in node_modules: run `bun install` first")
    data = Path(nodes[0]).read_bytes()
    a = data.find(START)
    if a < 0:
        sys.exit("the NSIS template was not found in the Tauri CLI binary (did the CLI change?)")
    z = data.find(END, a)
    if z < 0:
        sys.exit("the end of the NSIS template was not found")
    return data[a:z].decode("utf-8")


def build() -> str:
    text = upstream()
    for anchor, repl in PATCHES:
        n = text.count(anchor)
        if n != 1:
            sys.exit(f"patch anchor found {n} times (expected 1): {anchor.strip()[:60]!r}. Re-sync the patches with the new template.")
        text = text.replace(anchor, repl)
    return text


def main() -> int:
    want = build()
    if "--check" in sys.argv:
        have = OUT.read_text(encoding="utf-8") if OUT.exists() else ""
        if have != want:
            print(f"{OUT.relative_to(ROOT)} is out of date: run python3 scripts/make-nsis-template.py and review the diff", file=sys.stderr)
            return 1
        print("NSIS template is up to date")
        return 0
    OUT.write_text(want, encoding="utf-8")
    print(f"wrote {OUT.relative_to(ROOT)} ({len(want.splitlines())} lines)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
