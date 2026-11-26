import * as React from "react"
import { EyeIcon, EyeOffIcon, Loader2Icon } from "lucide-react"
import { toast } from "sonner"

import { FormFooter, FormHeader } from "@/components/form-layout"
import { Alert, AlertDescription } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { Dialog, DialogContent } from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { useUpdateGate } from "@/hooks/use-write-gate"
import { tk, useT } from "@/lib/i18n"
import { useStore } from "@/lib/store-context"
import type { AiProvider, AiProviderKind } from "@/lib/types"

export const PROVIDER_KIND_LABEL: Record<AiProviderKind, string> = {
  ninerouter: tk("9Router"),
  anthropic: tk("Anthropic"),
  openai: tk("OpenAI"),
  deepseek: tk("DeepSeek"),
  gemini: tk("Google Gemini (AI Studio)"),
}

const MODEL_HINT: Record<AiProviderKind, string> = {
  ninerouter: "nama-combo atau model",
  anthropic: "claude-sonnet-5-5",
  openai: "gpt-4.1-mini",
  deepseek: "deepseek-chat",
  gemini: "gemini-2.5-flash",
}

/** Model yang umum dipakai per jenis. Daftar bisa usang: pilihan "Lainnya" tetap menerima nama model apa pun. 9Router memakai nama combo sendiri, jadi hanya input manual. */
const MODEL_PRESETS: Record<AiProviderKind, string[]> = {
  ninerouter: [],
  anthropic: ["claude-fable-5-2", "claude-opus-5-5", "claude-sonnet-5-5", "claude-haiku-5-5"],
  openai: ["gpt-4.1", "gpt-4.1-mini", "gpt-4o", "gpt-4o-mini"],
  deepseek: ["deepseek-chat", "deepseek-reasoner"],
  gemini: ["gemini-2.5-pro", "gemini-2.5-flash", "gemini-2.5-flash-lite"],
}
const CUSTOM_MODEL = "__custom__"

interface Props {
  open: boolean
  onOpenChange: (open: boolean) => void
  provider?: AiProvider
  onSaved: () => void
}

export function AiProviderDialog({ open, onOpenChange, provider, onSaved }: Props) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[92svh] overflow-y-auto sm:max-w-md">
        {open && <AiProviderForm provider={provider} onSaved={onSaved} onClose={() => onOpenChange(false)} />}
      </DialogContent>
    </Dialog>
  )
}

export function AiProviderForm({ provider, onSaved, onClose }: { provider?: AiProvider; onSaved?: () => void; onClose: () => void }) {
  const t = useT()
  const { api } = useStore()
  const reason = useUpdateGate("pengaturan")()
  const editing = !!provider
  const [kind, setKind] = React.useState<AiProviderKind>(provider?.kind ?? "anthropic")
  const [name, setName] = React.useState(provider?.name ?? "")
  const [model, setModel] = React.useState(provider?.model ?? "")
  // Model di luar daftar bawaan (atau pilihan "Lainnya") ditulis manual.
  const [customModel, setCustomModel] = React.useState(() => !!provider?.model && !MODEL_PRESETS[provider.kind].includes(provider.model))
  const [baseUrl, setBaseUrl] = React.useState(provider?.baseUrl ?? "")
  const [apiKey, setApiKey] = React.useState("")
  const [priceIn, setPriceIn] = React.useState(provider?.priceInPerMtok === undefined ? "" : String(provider.priceInPerMtok))
  const [priceOut, setPriceOut] = React.useState(provider?.priceOutPerMtok === undefined ? "" : String(provider.priceOutPerMtok))
  const [show, setShow] = React.useState(false)
  const [busy, setBusy] = React.useState(false)
  const [error, setError] = React.useState<string | null>(null)

  const needsKey = true
  const needsUrl = kind === "ninerouter"
  const valid = name.trim() && model.trim() && (!needsKey || editing || apiKey.trim().length >= 8) && (!needsUrl || /^https?:\/\/\S+$/.test(baseUrl.trim())) && [priceIn, priceOut].every((v) => v.trim() === "" || (Number.isFinite(Number(v)) && Number(v) >= 0))

  async function submit(e: React.FormEvent) {
    e.preventDefault()
    if (!valid || busy || reason) return
    setBusy(true)
    setError(null)
    try {
      await api.saveAiProvider(provider?.id ?? null, {
        name,
        kind,
        model,
        baseUrl: needsUrl ? baseUrl : undefined,
        apiKey: needsKey && apiKey.trim() ? apiKey.trim() : undefined,
        priceInPerMtok: priceIn.trim() === "" ? undefined : Number(priceIn),
        priceOutPerMtok: priceOut.trim() === "" ? undefined : Number(priceOut),
      })
      toast.success(editing ? t("Provider disimpan") : t("Provider ditambahkan"))
      onSaved?.()
      onClose()
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  return (
    <form onSubmit={submit} className="grid gap-3">
      <FormHeader
        title={editing ? t("Edit provider AI") : t("Tambah provider AI")}
        description={t("Kunci API disimpan di keyring komputer dan tidak pernah ditampilkan lagi.")}
      />
      {reason && (
        <Alert>
          <AlertDescription>{reason}</AlertDescription>
        </Alert>
      )}
      <div className="grid gap-1.5">
        <Label htmlFor="pv-kind">{t("Jenis")}</Label>
        <Select
          value={kind}
          onValueChange={(v) => {
            setKind(v as AiProviderKind)
            setModel("")
            setCustomModel(false)
            if (!editing && !name.trim()) setName("")
          }}
          disabled={editing}
        >
          <SelectTrigger id="pv-kind" className="w-full">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {(Object.keys(PROVIDER_KIND_LABEL) as AiProviderKind[]).map((k) => (
              <SelectItem key={k} value={k}>
                {t(PROVIDER_KIND_LABEL[k]) /* i18n-ignore: kunci tk */}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
      <div className="grid gap-1.5">
        <Label htmlFor="pv-name">{t("Nama")}</Label>
        <Input id="pv-name" value={name} onChange={(e) => setName(e.target.value)} placeholder={t(PROVIDER_KIND_LABEL[kind]) /* i18n-ignore: kunci tk */} />
      </div>
      <div className="grid gap-1.5">
        <Label htmlFor="pv-model">{t("Model")}</Label>
        {MODEL_PRESETS[kind].length > 0 && (
          <Select
            value={customModel ? CUSTOM_MODEL : MODEL_PRESETS[kind].includes(model) ? model : ""}
            onValueChange={(v) => {
              if (v === CUSTOM_MODEL) {
                setCustomModel(true)
                setModel((m) => (MODEL_PRESETS[kind].includes(m) ? "" : m))
              } else {
                setCustomModel(false)
                setModel(v)
              }
            }}
          >
            <SelectTrigger id="pv-model" className="w-full font-mono">
              <SelectValue placeholder={t("Pilih model")} />
            </SelectTrigger>
            <SelectContent>
              {MODEL_PRESETS[kind].map((m) => (
                <SelectItem key={m} value={m} className="font-mono">
                  {m}
                </SelectItem>
              ))}
              <SelectItem value={CUSTOM_MODEL}>{t("Lainnya (ketik manual)")}</SelectItem>
            </SelectContent>
          </Select>
        )}
        {(MODEL_PRESETS[kind].length === 0 || customModel) && (
          <Input
            id={MODEL_PRESETS[kind].length === 0 ? "pv-model" : "pv-model-custom"}
            aria-label={t("Nama model")}
            className="font-mono"
            value={model}
            onChange={(e) => setModel(e.target.value)}
            placeholder={MODEL_HINT[kind]}
            autoCapitalize="off"
            spellCheck={false}
          />
        )}
      </div>
      {needsUrl && (
        <div className="grid gap-1.5">
          <Label htmlFor="pv-url">{t("URL dasar")}</Label>
          <Input id="pv-url" className="font-mono" value={baseUrl} onChange={(e) => setBaseUrl(e.target.value)} placeholder="http://10.0.0.5:8080/v1" inputMode="url" autoCapitalize="off" spellCheck={false} />
        </div>
      )}
      {needsKey && (
        <div className="grid gap-1.5">
          <Label htmlFor="pv-key">
            {t("Kunci API")}
            {editing && provider?.keyHint && <span className="ml-1 font-mono text-xs text-muted-foreground">({provider.keyHint})</span>}
          </Label>
          <div className="relative">
            <Input
              id="pv-key"
              type={show ? "text" : "password"}
              className="pr-10 font-mono"
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              placeholder={editing ? t("Kosongkan agar kunci tetap") : "sk-…"}
              autoComplete="off"
              spellCheck={false}
            />
            <button
              type="button"
              onClick={() => setShow((v) => !v)}
              aria-label={show ? t("Sembunyikan password") : t("Tampilkan password")}
              aria-pressed={show}
              className="absolute inset-y-0 right-0 flex w-10 items-center justify-center rounded-r-lg text-muted-foreground hover:text-foreground"
            >
              {show ? <EyeOffIcon className="size-4" /> : <EyeIcon className="size-4" />}
            </button>
          </div>
        </div>
      )}
      <div className="grid gap-1.5">
        <Label>{t("Harga per 1 juta token (USD, opsional)")}</Label>
        <div className="grid grid-cols-2 gap-2">
          <Input value={priceIn} onChange={(e) => setPriceIn(e.target.value)} inputMode="decimal" placeholder={t("Masuk")} aria-label={t("Harga token masuk per 1 juta (USD)")} className="font-mono" />
          <Input value={priceOut} onChange={(e) => setPriceOut(e.target.value)} inputMode="decimal" placeholder={t("Keluar")} aria-label={t("Harga token keluar per 1 juta (USD)")} className="font-mono" />
        </div>
        <p className="text-xs text-muted-foreground">{t("Dipakai untuk menghitung biaya dan anggaran. Kosong = biaya tidak dihitung (tetap tercatat jumlah token).")}</p>
      </div>
      {error && (
        <Alert variant="destructive">
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      )}
      <FormFooter>
        <Button type="button" variant="outline" onClick={onClose} disabled={busy}>
          {t("Batal")}
        </Button>
        <Button type="submit" disabled={!valid || busy || !!reason}>
          {busy && <Loader2Icon className="animate-spin" />}
          {t("Simpan")}
        </Button>
      </FormFooter>
    </form>
  )
}
