import { ChevronDownIcon } from "lucide-react"

import { Button } from "@/components/ui/button"
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { cn } from "@/lib/utils"

export interface FilterOption {
  value: string
  /** Teks di daftar pilihan. */
  label: string
  /** Teks pendek di tombol saat dipilih (bawaan: label). */
  short?: string
}

/**
 * Dropdown filter ringkas: tombol menampilkan nama filter (redup) saat nilai bawaan,
 * dan nilai terpilih (warna brand) saat filter aktif. Cocok untuk satu baris di HP.
 */
export function FilterSelect({
  label,
  value,
  defaultValue,
  options,
  onChange,
  className,
}: {
  label: string
  value: string
  defaultValue: string
  options: FilterOption[]
  onChange: (v: string) => void
  className?: string
}) {
  const active = value !== defaultValue
  const selected = options.find((o) => o.value === value)
  const text = active ? (selected?.short ?? selected?.label ?? label) : label
  return (
    <Select value={value} onValueChange={onChange}>
      <SelectTrigger
        aria-label={label}
        className={cn("min-w-0 flex-1", active ? "font-medium text-brand" : "text-muted-foreground", className)}
      >
        <SelectValue>{text}</SelectValue>
      </SelectTrigger>
      <SelectContent>
        {options.map((o) => (
          <SelectItem key={o.value} value={o.value}>
            {o.label}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  )
}

export interface MultiOption<T extends string> {
  value: T
  label: string
  icon?: React.ReactNode
}

/** Dropdown pilihan ganda (centang); tetap terbuka saat memilih. */
export function FilterMulti<T extends string>({
  label,
  values,
  options,
  onChange,
  className,
}: {
  label: string
  values: T[]
  options: MultiOption<T>[]
  onChange: (v: T[]) => void
  className?: string
}) {
  const n = values.length
  const text =
    n === 0 ? label : n === 1 ? (options.find((o) => o.value === values[0])?.label ?? label) : `${n} ${label.toLowerCase()}`
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          variant="outline"
          aria-label={label}
          className={cn(
            "min-w-0 flex-1 justify-between gap-1.5 px-2.5 font-normal",
            n > 0 ? "font-medium text-brand" : "text-muted-foreground",
            className
          )}
        >
          <span className="truncate">{text}</span>
          <ChevronDownIcon className="size-4 shrink-0 opacity-50" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" className="w-56">
        {options.map((o) => (
          <DropdownMenuCheckboxItem
            key={o.value}
            checked={values.includes(o.value)}
            onCheckedChange={(c) => onChange(c ? [...values, o.value] : values.filter((v) => v !== o.value))}
            onSelect={(e) => e.preventDefault()}
          >
            {o.icon}
            {o.label}
          </DropdownMenuCheckboxItem>
        ))}
        {n > 0 && (
          <>
            <DropdownMenuSeparator />
            <DropdownMenuItem onSelect={() => onChange([])}>Hapus filter</DropdownMenuItem>
          </>
        )}
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
