<script lang="ts">
  import Copy from '@lucide/svelte/icons/copy'
  import Check from '@lucide/svelte/icons/check'

  let {
    code,
    label = 'Copiar',
    wrap = false,
  }: {
    code: string
    label?: string
    /** Quebra linhas longas (tokens) em vez de rolar na horizontal. */
    wrap?: boolean
  } = $props()

  let copied = $state(false)

  async function copy() {
    await navigator.clipboard.writeText(code)
    copied = true
    setTimeout(() => (copied = false), 1500)
  }
</script>

<!-- min-w-0: dentro de grid/flex, sem ele uma linha longa alarga a página. -->
<div class="group relative min-w-0 rounded-md border bg-muted/40">
  <!-- Bloco que rola na horizontal precisa de foco para rolar com as setas
       (WCAG 2.1.1); por isso o tabindex num elemento não interativo. -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <pre
    tabindex={wrap ? undefined : 0}
    aria-label={wrap ? undefined : 'Código'}
    class={[
      'px-3 py-2.5 pr-10 font-mono text-xs leading-relaxed',
      wrap ? 'break-all whitespace-pre-wrap' : 'overflow-x-auto',
    ]}>{code}</pre>
  <button
    type="button"
    class="absolute top-1.5 right-1.5 grid size-7 place-items-center rounded-md text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
    aria-label={label}
    onclick={copy}
  >
    {#if copied}<Check class="size-3.5 text-brand" />{:else}<Copy class="size-3.5" />{/if}
  </button>
</div>
