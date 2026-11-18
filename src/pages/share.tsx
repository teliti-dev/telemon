import * as React from "react"
import { CopyIcon, GlobeIcon, Loader2Icon, ShieldAlertIcon, WifiIcon } from "lucide-react"
import { QRCodeSVG } from "qrcode.react"
import { toast } from "sonner"

import { AccountsCard } from "@/components/accounts-card"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Switch } from "@/components/ui/switch"
import { copyText } from "@/hooks/use-copy"
import { useNow } from "@/hooks/use-now"
import { formatDuration } from "@/lib/format"
import { navigate } from "@/lib/route"
import { useStore } from "@/lib/store-context"
import type { ShareBind } from "@/lib/types"

const errMsg = (e: unknown) => (e instanceof Error ? e.message : String(e))

function UrlBlock({ url, label }: { url: string; label: string }) {
  return (
    <div className="flex flex-col gap-3 sm:flex-row sm:items-start">
      <div className="w-fit shrink-0 rounded-lg border bg-white p-2">
        <QRCodeSVG value={url} size={112} marginSize={0} aria-label={`QR ${label}`} />
      </div>
      <div className="grid min-w-0 flex-1 gap-2">
        <Label className="text-xs text-muted-foreground">{label}</Label>
        <div className="flex gap-2">
          <Input readOnly value={url} className="font-mono text-xs" onFocus={(e) => e.target.select()} />
          <Button variant="outline" size="icon" aria-label="Salin URL" onClick={() => copyText(url, "URL disalin")}>
            <CopyIcon />
          </Button>
        </div>
        <p className="text-xs text-muted-foreground">
          Pindai QR dari HP atau bagikan URL ini. Pengunjung harus masuk dengan akun di bawah, dan hanya bisa melihat data.
        </p>
      </div>
    </div>
  )
}

export function SharePage() {
  const { share, api, access } = useStore()
  const now = useNow(1_000)
  const [bind, setBind] = React.useState<ShareBind>(share?.lan.bind ?? "lan")
  const [port, setPort] = React.useState(String(share?.lan.port ?? 7878))
  const [duration, setDuration] = React.useState(String(share?.publik.durationHours ?? 8))
  const [busy, setBusy] = React.useState<"lan" | "publik" | null>(null)

  if (!share) return null
  if (access === "remote") {
    return (
      <Alert className="max-w-lg">
        <ShieldAlertIcon />
        <AlertTitle>Hanya di aplikasi desktop</AlertTitle>
        <AlertDescription>
          Pengaturan berbagi dan akun jarak jauh hanya bisa diubah dari aplikasi Telemon di komputer.
          <Button variant="link" className="h-auto p-0 pl-1" onClick={() => navigate("ringkasan")}>
            Kembali ke Ringkasan
          </Button>
        </AlertDescription>
      </Alert>
    )
  }

  const lan = share.lan
  const pub = share.publik
  const portNum = Number(port)
  const portValid = Number.isInteger(portNum) && portNum >= 1024 && portNum <= 65535

  async function run(key: typeof busy, fn: () => Promise<unknown>, ok?: string) {
    setBusy(key)
    try {
      await fn()
      if (ok) toast.success(ok)
    } catch (e) {
      toast.error(errMsg(e))
    } finally {
      setBusy(null)
    }
  }

  return (
    <div className="grid items-start gap-3 xl:grid-cols-2">
      {/* Serve ke browser (LAN) */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <WifiIcon className="size-4" /> Serve ke browser
          </CardTitle>
          <CardDescription>Buka dashboard dari browser lain atau HP di jaringan yang sama.</CardDescription>
          <CardAction>
            <Switch
              aria-label="Serve ke browser"
              checked={lan.enabled}
              disabled={busy === "lan" || (!lan.enabled && !portValid)}
              onCheckedChange={(v) =>
                run("lan", () => api.setLanShare({ enabled: v, bind, port: portNum }), v ? "Server lokal aktif" : "Server lokal dimatikan")
              }
            />
          </CardAction>
        </CardHeader>
        <CardContent className="grid gap-3.5">
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-[1fr_8rem]">
            <div className="grid gap-2">
              <Label htmlFor="bind">Bisa dibuka dari</Label>
              <Select value={bind} onValueChange={(v) => setBind(v as ShareBind)} disabled={lan.enabled}>
                <SelectTrigger id="bind" className="w-full">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="lan">Jaringan lokal (LAN/Wi-Fi)</SelectItem>
                  <SelectItem value="localhost">Hanya komputer ini</SelectItem>
                </SelectContent>
              </Select>
            </div>
            <div className="grid gap-2">
              <Label htmlFor="port">Port</Label>
              <Input
                id="port"
                inputMode="numeric"
                value={port}
                disabled={lan.enabled}
                aria-invalid={!portValid}
                onChange={(e) => setPort(e.target.value)}
              />
            </div>
          </div>
          {!portValid && <p className="-mt-2 text-xs text-destructive">Port harus 1024-65535.</p>}

          {lan.enabled && lan.url ? (
            <UrlBlock url={lan.url} label="URL (masuk dengan akun)" />
          ) : (
            <p className="text-sm text-muted-foreground">
              Mati. Saat dinyalakan, Telemon membuka port {portValid ? portNum : "-"} di komputer ini; Windows bisa meminta izin
              firewall. Pengunjung masuk dengan akun jarak jauh di bawah.
            </p>
          )}
        </CardContent>
      </Card>

      {/* Publik (Cloudflare Quick Tunnel) */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <GlobeIcon className="size-4" /> Publik (internet)
            {pub.status === "on" && <Badge variant="outline">Aktif</Badge>}
          </CardTitle>
          <CardDescription>Buka dashboard dari mana saja lewat Cloudflare Quick Tunnel, tanpa akun Cloudflare.</CardDescription>
          <CardAction>
            <Switch
              aria-label="Akses publik"
              checked={pub.status === "on" || pub.status === "connecting"}
              disabled={busy === "publik" || pub.status === "connecting" || (pub.status === "off" && !lan.enabled)}
              onCheckedChange={(v) =>
                run("publik", () => api.setPublicShare({ enabled: v, durationHours: Number(duration) }), v ? "Akses publik aktif" : "Akses publik dimatikan")
              }
            />
          </CardAction>
        </CardHeader>
        <CardContent className="grid gap-3.5">
          <Alert>
            <ShieldAlertIcon />
            <AlertTitle>Data pelanggan bisa diakses dari internet</AlertTitle>
            <AlertDescription>
              Username, IP, dan MAC pelanggan terlihat oleh akun yang berhasil masuk. Selama akses publik aktif, 2FA
              diwajibkan untuk semua akun. Nyalakan hanya saat perlu.
            </AlertDescription>
          </Alert>

          <div className="grid gap-2">
            <Label htmlFor="duration">Mati otomatis setelah</Label>
            <Select value={duration} onValueChange={setDuration} disabled={pub.status !== "off"}>
              <SelectTrigger id="duration" className="w-full sm:w-48">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {[1, 4, 8, 24].map((h) => (
                  <SelectItem key={h} value={String(h)}>
                    {h} jam
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          {pub.status === "off" && !lan.enabled && (
            <p className="text-sm text-muted-foreground">Syarat: nyalakan Serve ke browser terlebih dulu.</p>
          )}

          {pub.status === "connecting" && (
            <p className="flex items-center gap-2 text-sm text-muted-foreground">
              <Loader2Icon className="size-4 animate-spin" /> Menghubungkan ke Cloudflare…
            </p>
          )}

          {pub.status === "error" && pub.error && (
            <Alert variant="destructive">
              <AlertDescription>{pub.error}</AlertDescription>
            </Alert>
          )}

          {pub.status === "on" && pub.url && (
            <>
              <UrlBlock url={pub.url} label="URL publik (masuk dengan akun dan 2FA)" />
              <p className="text-sm text-muted-foreground">
                {pub.expiresAt && (
                  <>
                    Mati otomatis dalam{" "}
                    <span className="font-medium text-foreground tabular-nums">{formatDuration(pub.expiresAt - now)}</span>.{" "}
                  </>
                )}
                URL berganti setiap kali akses publik dinyalakan.
              </p>
            </>
          )}
        </CardContent>
      </Card>

      <AccountsCard />
    </div>
  )
}
