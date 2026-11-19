import { BugIcon, CopyIcon, ExternalLinkIcon, CodeIcon, GlobeIcon, PackageIcon, ZapIcon } from "lucide-react"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { copyText } from "@/hooks/use-copy"
import { APP_VERSION, COPYRIGHT, ISSUES_URL, LICENSE_URL, ORG_NAME, ORG_URL, RELEASES_PAGE_URL, REPO_URL, THIRD_PARTY } from "@/lib/app-info"
import { useLang, useT } from "@/lib/i18n"
import { useStore } from "@/lib/store-context"

function LinkButton({ href, icon, children }: { href: string; icon: React.ReactNode; children: React.ReactNode }) {
  return (
    <Button variant="outline" size="sm" asChild>
      <a href={href} target="_blank" rel="noopener noreferrer">
        {icon}
        {children}
        <ExternalLinkIcon className="opacity-60" />
      </a>
    </Button>
  )
}

/** Halaman Tentang: versi, tautan, lisensi, dan info diagnostik yang aman dibagikan (tanpa data pengguna). */
export function AboutPage() {
  const t = useT()
  const lang = useLang()
  const { access } = useStore()
  const accessLabel = access === "desktop" ? t("Aplikasi desktop") : t("Browser")
  const rows: [string, string][] = [
    [t("Versi"), APP_VERSION],
    [t("Akses"), accessLabel],
    [t("Bahasa"), lang === "id" ? "Indonesia" : "English"],
    [t("Peramban"), navigator.userAgent],
  ]
  const diagnostic = [`Telemon ${APP_VERSION}`, ...rows.slice(1).map(([k, v]) => `${k}: ${v}`)].join("\n")

  return (
    <div className="grid max-w-3xl gap-4">
      <Card>
        <CardHeader>
          <div className="flex items-center gap-3">
            <div className="flex size-12 items-center justify-center rounded-xl bg-foreground text-background">
              <ZapIcon className="size-6" />
            </div>
            <div className="min-w-0">
              <CardTitle className="flex items-center gap-2 text-xl">
                Telemon
                <Badge variant="secondary" className="font-mono">{APP_VERSION}</Badge>
              </CardTitle>
              <CardDescription>{t("Pemantau user dan event PPPoE di banyak router Mikrotik.")}</CardDescription>
              <p className="mt-1 text-xs text-muted-foreground">
                {COPYRIGHT} · {t("Dikembangkan oleh")}{" "}
                <a href={ORG_URL} target="_blank" rel="noopener noreferrer" className="underline underline-offset-2 hover:text-foreground">{ORG_NAME}</a>
              </p>
            </div>
          </div>
        </CardHeader>
        <CardContent className="flex flex-wrap gap-2">
          <LinkButton href={REPO_URL} icon={<CodeIcon />}>{t("Kode sumber")}</LinkButton>
          <LinkButton href={RELEASES_PAGE_URL} icon={<PackageIcon />}>{t("Rilis")}</LinkButton>
          <LinkButton href={ISSUES_URL} icon={<BugIcon />}>{t("Laporkan masalah")}</LinkButton>
          <LinkButton href={ORG_URL} icon={<GlobeIcon />}>{ORG_NAME}</LinkButton>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>{t("Info diagnostik")}</CardTitle>
          <CardDescription>{t("Sertakan ini saat melaporkan masalah. Tidak memuat data router, akun, atau kata sandi.")}</CardDescription>
        </CardHeader>
        <CardContent className="grid gap-3">
          <dl className="grid gap-2 text-sm sm:grid-cols-[8rem_1fr]">
            {rows.map(([k, v]) => (
              <div key={k} className="contents">
                <dt className="text-muted-foreground">{k}</dt>
                <dd className="min-w-0 break-words font-mono text-xs sm:text-sm">{v}</dd>
              </div>
            ))}
          </dl>
          <div>
            <Button variant="outline" size="sm" onClick={() => copyText(diagnostic, t("Info diagnostik disalin"))}>
              <CopyIcon />
              {t("Salin info")}
            </Button>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>{t("Lisensi")}</CardTitle>
          <CardDescription>
            {t("Telemon adalah perangkat lunak bebas berlisensi GNU AGPL-3.0. Kamu boleh memakai, mempelajari, mengubah, dan membagikannya; versi yang dimodifikasi dan disediakan lewat jaringan harus menyertakan kode sumbernya.")}
          </CardDescription>
        </CardHeader>
        <CardContent className="grid gap-4">
          <div>
            <LinkButton href={LICENSE_URL} icon={null}>{t("Baca teks lisensi")}</LinkButton>
          </div>
          <div>
            <h3 className="mb-2 text-sm font-medium">{t("Komponen pihak ketiga")}</h3>
            <ul className="grid gap-1 text-sm sm:grid-cols-2">
              {THIRD_PARTY.map((c) => (
                <li key={c.name} className="flex justify-between gap-3 border-b py-1 last:border-0">
                  <span>{c.name}</span>
                  <span className="text-muted-foreground">{c.license}</span>
                </li>
              ))}
            </ul>
          </div>
        </CardContent>
      </Card>
    </div>
  )
}
