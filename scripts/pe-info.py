#!/usr/bin/env python3
"""Read the metadata of a Windows .exe (installer or app) without Windows: version strings, icon sizes, signature.

  python3 scripts/pe-info.py FILE.exe                                    print the metadata
  python3 scripts/pe-info.py FILE.exe --require CompanyName,Comments     fail if a version string is missing or empty
  python3 scripts/pe-info.py FILE.exe --icon-size 256                    fail if no icon of that size is embedded
"""
import struct
import sys
from pathlib import Path


def rsrc(d):
    pe = struct.unpack_from("<I", d, 0x3C)[0]
    if d[pe : pe + 4] != b"PE\0\0":
        sys.exit("not a PE file")
    nsec = struct.unpack_from("<H", d, pe + 6)[0]
    opt = pe + 24
    plus = struct.unpack_from("<H", d, opt)[0] == 0x20B
    dd = opt + (112 if plus else 96)
    rva = struct.unpack_from("<I", d, dd + 16)[0]
    sig = struct.unpack_from("<I", d, dd + 36)[0]
    sect = opt + struct.unpack_from("<H", d, pe + 20)[0]
    secs = []
    for i in range(nsec):
        o = sect + 40 * i
        vs, va, rs, rp = struct.unpack_from("<IIII", d, o + 8)
        secs.append((va, vs, rp, rs))

    def off(r):
        for va, vs, rp, rs in secs:
            if va <= r < va + max(vs, rs):
                return rp + r - va

    return (off(rva) if rva else None), off, sig


def entries(d, o):
    nn, ni = struct.unpack_from("<HH", d, o + 12)
    return [struct.unpack_from("<II", d, o + 16 + 8 * i) for i in range(nn + ni)]


def leaves(d, base, off, o, t):
    """Yield the data blob of every leaf under resource directory `o` (type `t` only at the top)."""
    for ident, dat in entries(d, o):
        if t is not None and ident != t:
            continue
        sub = base + (dat & 0x7FFFFFFF)
        stack = [sub]
        while stack:
            cur = stack.pop()
            for _, dat2 in entries(d, cur):
                if dat2 & 0x80000000:
                    stack.append(base + (dat2 & 0x7FFFFFFF))
                else:
                    rva, sz, _, _ = struct.unpack_from("<IIII", d, base + dat2)
                    yield d[off(rva) : off(rva) + sz]


def version_strings(blob):
    """Parse a VS_VERSIONINFO resource and return its string table as {name: value}."""
    out = {}

    def parse(p, end):
        while p + 6 <= end:
            length, vlen, typ = struct.unpack_from("<HHH", blob, p)
            if length == 0:
                break
            e = p + 6
            while blob[e : e + 2] != b"\0\0":
                e += 2
            key = blob[p + 6 : e].decode("utf-16-le")
            body = (e + 2 + 3) & ~3
            nxt = p + length
            if key == "VS_VERSION_INFO":
                parse((body + vlen + 3) & ~3, nxt)  # skip the fixed file info
            elif key == "StringFileInfo" or (len(key) == 8 and all(c in "0123456789abcdefABCDEF" for c in key)):
                parse(body, nxt)  # the string table
            elif typ == 1 and vlen:
                out[key] = blob[body : body + vlen * 2].decode("utf-16-le").rstrip("\0")
            p = (nxt + 3) & ~3

    parse(0, len(blob))
    return out


def main():
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    f = Path(sys.argv[1])
    d = f.read_bytes()
    base, off, sig = rsrc(d)
    strings, icons = {}, set()
    if base:
        for blob in leaves(d, base, off, base, 16):
            strings.update(version_strings(blob))
        for g in leaves(d, base, off, base, 14):
            n = struct.unpack_from("<H", g, 4)[0]
            icons |= {(g[6 + 14 * i] or 256) for i in range(n)}
    print(f"{f.name}: {len(d) // 1024} KB, Authenticode signature: {'yes' if sig else 'no'}")
    for k in ("FileDescription", "ProductName", "CompanyName", "LegalCopyright", "FileVersion", "ProductVersion", "Comments"):
        print(f"  {k:16} {strings.get(k, '(missing)')}")
    print(f"  icon sizes       {sorted(icons)}")
    ok = True
    for flag, items in (("--require", None), ("--icon-size", None)):
        if flag in sys.argv:
            items = sys.argv[sys.argv.index(flag) + 1].split(",")
            if flag == "--require":
                for k in items:
                    if not strings.get(k):
                        print(f"MISSING version string: {k}", file=sys.stderr)
                        ok = False
            else:
                for s in items:
                    if int(s) not in icons:
                        print(f"MISSING icon size: {s}", file=sys.stderr)
                        ok = False
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
