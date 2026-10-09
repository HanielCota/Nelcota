<script lang="ts">
  // A part of a whole as a bar with both numbers under it (D98). The fill is
  // the brand colour; the track is a lighter step of it.
  let { label, part, whole, format }: { label: string; part: number; whole: number; format: (n: number) => string } = $props()
  const share = $derived(whole > 0 ? Math.min(100, (part / whole) * 100) : 0)
</script>

<div class="grid min-w-36 gap-2">
  <p class="text-sm text-muted-foreground">{label}</p>
  <div
    class="h-2 overflow-hidden rounded-full bg-brand/15"
    role="meter"
    aria-label={label}
    aria-valuemin={0}
    aria-valuemax={whole}
    aria-valuenow={part}
  >
    <div class="h-full rounded-full bg-brand" style:width={`${share}%`}></div>
  </div>
  <p class="flex justify-between gap-4 text-xs tabular-nums">
    <span class="font-medium">{format(part)}</span>
    <span class="text-muted-foreground">{format(whole)}</span>
  </p>
</div>
