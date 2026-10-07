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
<section class="rounded-lg border bg-muted/30">
  <header class="flex items-center gap-2 px-3 py-2">
    <button
      type="button"
      class="flex flex-1 items-center gap-1.5 text-left text-xs font-medium text-muted-foreground hover:text-foreground"
      aria-expanded={open}
      onclick={() => (open = !open)}
    >
      <ChevronRight class={['size-3.5 transition-transform', open && 'rotate-90']} />
      SQL que será executado
      {#if preview.loading}<span class="font-normal">· atualizando…</span>{/if}
    </button>
    {#if text}
      <button
        type="button"
        class="grid size-6 place-items-center rounded text-muted-foreground hover:bg-accent hover:text-foreground"
        aria-label="Copiar SQL"
        onclick={copy}><Copy class="size-3.5" /></button
      >
    {/if}
  </header>
  {#if preview.error}
    <p class="border-t px-3 py-2 text-xs text-destructive">{preview.error}</p>
  {:else if open}
    <pre class="max-h-64 overflow-auto border-t px-3 py-2 font-mono text-2xs leading-relaxed whitespace-pre-wrap">{text ||
        placeholder}</pre>
  {/if}
</section>
