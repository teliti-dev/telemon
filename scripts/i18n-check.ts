/**
 * Pemeriksa terjemahan: `bun run i18n:check [berkas...]`.
 * 1. Teks tampilan yang belum dibungkus t() (teks JSX, atribut tampilan, toast, Error).
 * 2. Kunci t("...")/tk("...") yang belum ada di kamus English.
 * Keluar dengan kode 1 bila ada temuan.
 */
import { readdirSync, readFileSync, statSync } from "node:fs"
import { join, relative, resolve } from "node:path"
import ts from "typescript"

import { EN } from "../src/lib/i18n/en"

const ROOT = resolve(import.meta.dirname, "../src")
const SKIP = ["components/ui/", "lib/i18n/", "lib/types.ts", "main.tsx", "vite-env.d.ts"]
const ATTRS = new Set(["title", "placeholder", "aria-label", "alt", "label", "description", "summary", "hint"])
const BRAND = ["Telemon", "Mikrotik", "RouterOS", "Cloudflare", "Quick Tunnel", "TOTP", "2FA", "API", "SSL", "TLS", "IP", "MAC", "PPPoE", "PPP", "OK", "LAN", "SSE", "WebSocket", "Live"]

function walk(dir: string): string[] {
  return readdirSync(dir).flatMap((f) => {
    const p = join(dir, f)
    return statSync(p).isDirectory() ? walk(p) : /\.tsx?$/.test(p) ? [p] : []
  })
}

const files = (process.argv.slice(2).length ? process.argv.slice(2).map((f) => resolve(f)) : walk(ROOT)).filter(
  (f) => !SKIP.some((s) => relative(ROOT, f).startsWith(s) || relative(ROOT, f) === s)
)

function human(s: string) {
  let x = s.replace(/\s+/g, " ").trim()
  for (const b of BRAND) x = x.split(b).join(" ")
  x = x.replace(/\/[\w./-]+/g, " ") // path seperti /ppp/secret
  return /[A-Za-z]{3,}/.test(x) && !/^[a-z0-9_:./#\-[\]()]+$/.test(s.trim())
}

const untranslated: string[] = []
const keys = new Map<string, string>()

for (const file of files) {
  const text = readFileSync(file, "utf8")
  const sf = ts.createSourceFile(file, text, ts.ScriptTarget.Latest, true, file.endsWith("x") ? ts.ScriptKind.TSX : ts.ScriptKind.TS)
  const rel = relative(ROOT, file)
  const lines = text.split("\n")
  const ignored = (n: ts.Node) => {
    const i = sf.getLineAndCharacterOfPosition(n.getStart()).line
    return /i18n-ignore/.test(lines[i] ?? "") || /i18n-ignore/.test(lines[i - 1] ?? "")
  }
  const at = (n: ts.Node) => `${rel}:${sf.getLineAndCharacterOfPosition(n.getStart()).line + 1}`
  const lit = (n?: ts.Node) => n && (ts.isStringLiteral(n) || ts.isNoSubstitutionTemplateLiteral(n)) ? n.text : undefined

  const visit = (n: ts.Node) => {
    if (ignored(n)) {
      ts.forEachChild(n, visit)
      return
    }
    if (ts.isJsxText(n)) {
      const s = n.getText().replace(/\s+/g, " ").trim()
      if (s && human(s)) untranslated.push(`${at(n)}  teks JSX: ${s.slice(0, 70)}`)
    } else if (ts.isJsxAttribute(n) && ATTRS.has(n.name.getText()) && n.initializer) {
      const v = ts.isStringLiteral(n.initializer) ? n.initializer.text : ts.isJsxExpression(n.initializer) ? lit(n.initializer.expression) : undefined
      if (v && human(v)) untranslated.push(`${at(n)}  atribut ${n.name.getText()}: ${v.slice(0, 70)}`)
    } else if (ts.isJsxExpression(n) && n.parent && (ts.isJsxElement(n.parent) || ts.isJsxFragment(n.parent))) {
      const v = lit(n.expression)
      if (v && human(v)) untranslated.push(`${at(n)}  string dalam JSX: ${v.slice(0, 70)}`)
    } else if (ts.isCallExpression(n)) {
      const callee = n.expression.getText()
      if (callee === "t" || callee === "tk") {
        const v = lit(n.arguments[0])
        if (v === undefined) untranslated.push(`${at(n)}  ${callee}() harus berisi string literal`)
        else keys.set(v, at(n))
      } else if (/^toast\.(success|error|info|warning|message)$/.test(callee)) {
        const a = n.arguments[0]
        if (a && (lit(a) !== undefined || ts.isTemplateExpression(a))) untranslated.push(`${at(n)}  toast belum t(): ${a.getText().slice(0, 70)}`)
      }
    } else if (ts.isNewExpression(n) && n.expression.getText() === "Error") {
      const a = n.arguments?.[0]
      if (a && (lit(a) !== undefined || ts.isTemplateExpression(a))) untranslated.push(`${at(n)}  Error belum t(): ${a.getText().slice(0, 70)}`)
    }
    ts.forEachChild(n, visit)
  }
  visit(sf)
}

// Mutu kamus: placeholder sama, nilai tidak kosong, tidak ada kunci ganda dengan terjemahan berbeda.
const ph = (x: string) => [...x.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort().join(",")
const badPlaceholders: string[] = []
const empty: string[] = []
for (const [k, v] of Object.entries(EN)) {
  if (!v.trim()) empty.push(k)
  for (const half of v.split("|")) {
    if (ph(half) !== ph(k) && !(v.includes("|") && ph(half) === "" && ph(k) === "")) badPlaceholders.push(`"${k}" -> "${v}"`)
  }
}
const dir = resolve(import.meta.dirname, "../src/lib/i18n/en")
const seen = new Map<string, { v: string; area: string }>()
const conflicts: string[] = []
for (const f of readdirSync(dir).filter((x) => x.endsWith(".ts") && x !== "index.ts")) {
  const mod = (await import(join(dir, f))).default as Record<string, string>
  for (const [k, v] of Object.entries(mod)) {
    const prev = seen.get(k)
    if (prev && prev.v !== v) conflicts.push(`"${k}": ${prev.area}="${prev.v}" vs ${f}="${v}"`)
    else seen.set(k, { v, area: f })
  }
}

const missing = [...keys.entries()].filter(([k]) => !(k in EN))
const same = [...keys.entries()].filter(([k]) => k in EN && EN[k] === k && /[A-Za-z]{4,}/.test(k)).length

console.log(`Berkas diperiksa: ${files.length}; kunci t()/tk(): ${keys.size}`)
if (untranslated.length) {
  console.log(`\nBELUM DIBUNGKUS t() (${untranslated.length}):`)
  untranslated.forEach((l) => console.log("  " + l))
}
if (missing.length) {
  console.log(`\nKUNCI BELUM ADA DI KAMUS EN (${missing.length}):`)
  missing.forEach(([k, loc]) => console.log(`  ${loc}  "${k}"`))
}
if (badPlaceholders.length) {
  console.log(`\nPLACEHOLDER TIDAK SAMA (${badPlaceholders.length}):`)
  badPlaceholders.forEach((l) => console.log("  " + l))
}
if (empty.length) console.log(`\nNILAI KOSONG (${empty.length}): ${empty.join("; ")}`)
if (conflicts.length) {
  console.log(`\nKUNCI GANDA DENGAN TERJEMAHAN BERBEDA (${conflicts.length}):`)
  conflicts.forEach((l) => console.log("  " + l))
}
if (same) console.log(`\n(info) ${same} kunci bernilai sama dengan kuncinya di kamus EN`)
process.exit(untranslated.length || missing.length || badPlaceholders.length || empty.length || conflicts.length ? 1 : 0)
