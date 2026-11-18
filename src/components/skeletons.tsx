import { Card, CardContent, CardHeader } from "@/components/ui/card"
import { Skeleton } from "@/components/ui/skeleton"
import type { Page } from "@/lib/route"
import { cn } from "@/lib/utils"

/** Baris daftar dengan shimmer. */
export function SkeletonRows({ count = 6, className }: { count?: number; className?: string }) {
  return (
    <ul className={cn("divide-y", className)} aria-hidden>
      {Array.from({ length: count }, (_, i) => (
        <li key={i} className="flex items-center gap-3 px-3 py-2.5">
          <Skeleton className="size-4 shrink-0 rounded-full" />
          <div className="grid min-w-0 flex-1 gap-1.5">
            <Skeleton className="h-3.5 w-2/5" />
            <Skeleton className="h-3 w-3/5" />
          </div>
          <Skeleton className="h-3.5 w-14 shrink-0" />
        </li>
      ))}
    </ul>
  )
}

/** Kartu grid dengan shimmer. */
export function SkeletonCards({ count = 6 }: { count?: number }) {
  return (
    <ul className="grid grid-cols-1 gap-2 min-[420px]:grid-cols-2 md:grid-cols-3 xl:grid-cols-4 2xl:grid-cols-5" aria-hidden>
      {Array.from({ length: count }, (_, i) => (
        <li key={i} className="grid gap-2 rounded-xl border p-3">
          <div className="flex items-center justify-between gap-2">
            <Skeleton className="h-4 w-1/2" />
            <Skeleton className="h-5 w-16 rounded-full" />
          </div>
          <Skeleton className="h-3 w-4/5" />
          <div className="grid grid-cols-2 gap-2">
            <Skeleton className="h-8" />
            <Skeleton className="h-8" />
          </div>
        </li>
      ))}
    </ul>
  )
}

function ToolbarSkeleton() {
  return (
    <div className="grid gap-2" aria-hidden>
      <Skeleton className="h-9 w-full md:h-7 md:w-72" />
      <div className="flex gap-2">
        <Skeleton className="h-9 flex-1 md:h-7 md:w-40 md:flex-none" />
        <Skeleton className="h-9 flex-1 md:h-7 md:w-40 md:flex-none" />
        <Skeleton className="hidden h-7 w-24 md:block" />
      </div>
    </div>
  )
}

function CardBlock({ lines = 4, title = true }: { lines?: number; title?: boolean }) {
  return (
    <Card>
      {title && (
        <CardHeader>
          <Skeleton className="h-4 w-40" />
          <Skeleton className="h-3 w-64 max-w-full" />
        </CardHeader>
      )}
      <CardContent className="grid gap-3">
        {Array.from({ length: lines }, (_, i) => (
          <div key={i} className="grid gap-1.5">
            <Skeleton className="h-3 w-28" />
            <Skeleton className="h-9 w-full md:h-7" />
          </div>
        ))}
      </CardContent>
    </Card>
  )
}

function OverviewSkeleton() {
  return (
    <div className="flex flex-col gap-3" aria-hidden>
      <div className="grid grid-cols-2 gap-2 md:gap-3 lg:grid-cols-5">
        {Array.from({ length: 5 }, (_, i) => (
          <Card key={i} className={cn("gap-2 py-2.5", i === 0 && "col-span-2 lg:col-span-1")}>
            <CardContent className="grid gap-2 px-3">
              <Skeleton className="h-3 w-24" />
              <Skeleton className="h-6 w-16" />
            </CardContent>
          </Card>
        ))}
      </div>
      <div className="grid gap-2 sm:grid-cols-2 md:gap-3 xl:grid-cols-4">
        {Array.from({ length: 4 }, (_, i) => (
          <Card key={i} className="gap-2 py-2.5">
            <CardContent className="grid gap-2 px-3">
              <Skeleton className="h-4 w-28" />
              <Skeleton className="h-3 w-20" />
              <Skeleton className="h-2 w-full rounded-full" />
            </CardContent>
          </Card>
        ))}
      </div>
      <div className="grid gap-3 lg:grid-cols-3">
        <Card className="gap-0 py-0 lg:col-span-2">
          <SkeletonRows count={6} />
        </Card>
        <Card className="gap-0 py-0">
          <SkeletonRows count={5} />
        </Card>
      </div>
    </div>
  )
}

/** Kerangka loading per halaman: bentuknya meniru halaman aslinya. */
export function PageSkeleton({ page }: { page: Page }) {
  switch (page) {
    case "ringkasan":
      return <OverviewSkeleton />
    case "user":
      return (
        <div className="flex flex-col gap-3">
          <ToolbarSkeleton />
          <Card className="gap-0 py-0">
            <SkeletonRows count={10} />
          </Card>
        </div>
      )
    case "profile":
    case "event":
      return (
        <div className="flex flex-col gap-3">
          <ToolbarSkeleton />
          <Card className="gap-0 py-0">
            <SkeletonRows count={12} />
          </Card>
        </div>
      )
    case "router":
      return (
        <div className="grid gap-2 md:grid-cols-2 md:gap-3 xl:grid-cols-3" aria-hidden>
          {Array.from({ length: 4 }, (_, i) => (
            <CardBlock key={i} lines={3} />
          ))}
        </div>
      )
    default:
      return (
        <div className="grid items-start gap-3 xl:grid-cols-2" aria-hidden>
          <CardBlock lines={3} />
          <CardBlock lines={3} />
        </div>
      )
  }
}

/** Penanda "memuat lagi" di akhir daftar untuk scroll tanpa henti. */
export function LoadMore({
  sentinelRef,
  variant = "rows",
}: {
  sentinelRef: React.RefObject<HTMLDivElement | null>
  variant?: "rows" | "cards"
}) {
  return (
    <div ref={sentinelRef} role="status" aria-busy="true">
      {variant === "cards" ? <SkeletonCards count={3} /> : <SkeletonRows count={2} className="rounded-xl border" />}
    </div>
  )
}
