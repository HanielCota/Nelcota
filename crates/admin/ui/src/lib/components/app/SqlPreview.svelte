<script lang="ts">
  import Copy from '@lucide/svelte/icons/copy'
  import ChevronRight from '@lucide/svelte/icons/chevron-right'
  import { toast } from 'svelte-sonner'
  import type { SqlPreview } from '$lib/preview.svelte'

  let { preview, placeholder = 'Preencha o formulário para ver o SQL.' }: { preview: SqlPreview; placeholder?: string } =
    $props()

  let open = $state(false)
  const text = $derived(preview.sql?.map((s) => `${s};`).join('\n\n') ?? '')

  async function copy() {
    await navigator.clipboard.writeText(text)
    toast.success('SQL copiado')
  }
</script>

<!-- O SQL que será executado, gerado pelo servidor. -->
<section class="overflow-hidden rounded-xl border bg-muted/30">
  <header class="flex items-center gap-2 px-4 py-2.5">
    <button
      type="button"
      class="flex flex-1 cursor-pointer items-center gap-2 text-left text-sm font-semibold text-muted-foreground transition-colors hover:text-foreground"
      aria-expanded={open}
      onclick={() => (open = !open)}
    >
      <ChevronRight class={['size-4 transition-transform', open && 'rotate-90']} />
      SQL que será executado
      {#if preview.loading}<span class="text-xs font-normal">· atualizando…</span>{/if}
    </button>
    {#if text}
      <button
        type="button"
        class="grid size-8 cursor-pointer place-items-center rounded-md text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
        aria-label="Copiar SQL"
        onclick={copy}><Copy class="size-4" /></button
      >
    {/if}
  </header>
  {#if preview.error}
    <p class="border-t border-destructive/20 bg-destructive/5 px-4 py-2.5 text-sm text-destructive">{preview.error}</p>
  {:else if open}
    <pre class="max-h-72 overflow-auto border-t bg-card/60 px-4 py-3 font-mono text-xs leading-relaxed whitespace-pre-wrap">{text ||
        placeholder}</pre>
  {/if}
</section>
