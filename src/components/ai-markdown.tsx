import * as React from "react"

/** Menampilkan jawaban AI: paragraf, daftar "- ", **tebal**, dan `kode`. Tanpa HTML mentah. */
function inline(text: string, prefix: string): React.ReactNode[] {
  return text.split(/(\*\*[^*]+\*\*|`[^`]+`)/g).map((part, i) => {
    const key = `${prefix}-${i}`
    if (part.length > 4 && part.startsWith("**") && part.endsWith("**")) {
      return (
        <strong key={key} className="font-semibold">
          {part.slice(2, -2)}
        </strong>
      )
    }
    if (part.length > 2 && part.startsWith("`") && part.endsWith("`")) {
      return (
        <code key={key} className="rounded bg-background/70 px-1 py-0.5 font-mono text-[0.85em]">
          {part.slice(1, -1)}
        </code>
      )
    }
    return <React.Fragment key={key}>{part}</React.Fragment>
  })
}

export function AiMarkdown({ text }: { text: string }) {
  const blocks: React.ReactNode[] = []
  let list: string[] = []
  const flush = (i: number) => {
    if (!list.length) return
    blocks.push(
      <ul key={`ul-${i}`} className="my-1 list-disc space-y-0.5 pl-5">
        {list.map((item, j) => (
          <li key={j}>{inline(item, `li-${i}-${j}`)}</li>
        ))}
      </ul>
    )
    list = []
  }
  text.split("\n").forEach((line, i) => {
    if (line.startsWith("- ")) {
      list.push(line.slice(2))
      return
    }
    flush(i)
    if (line.trim()) {
      blocks.push(
        <p key={`p-${i}`} className="my-1">
          {inline(line, `p-${i}`)}
        </p>
      )
    }
  })
  flush(text.split("\n").length)
  return <div className="text-sm leading-relaxed break-words">{blocks}</div>
}
