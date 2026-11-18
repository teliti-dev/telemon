import { cn } from "@/lib/utils"

/** Placeholder loading dengan sapuan cahaya (shimmer). Nonaktif bila pengguna memilih mengurangi gerakan. */
function Skeleton({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="skeleton"
      aria-hidden
      className={cn(
        "relative overflow-hidden rounded-md bg-muted",
        "after:absolute after:inset-0 after:-translate-x-full after:animate-[shimmer_1.6s_ease-in-out_infinite] after:bg-gradient-to-r after:from-transparent after:via-foreground/[0.07] after:to-transparent",
        "motion-reduce:after:hidden",
        className
      )}
      {...props}
    />
  )
}

export { Skeleton }
