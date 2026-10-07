<script lang="ts">
  import Copy from '@lucide/svelte/icons/copy'
  import Check from '@lucide/svelte/icons/check'
  import { t } from '$lib/i18n/index.svelte'

  let {
    code,
    label,
    wrap = false,
  }: {
    code: string
    label?: string
    /** Wraps long lines (tokens) instead of scrolling horizontally. */
    wrap?: boolean
  } = $props()

  let copied = $state(false)

  async function copy() {
    await navigator.clipboard.writeText(code)
    copied = true
    setTimeout(() => (copied = false), 1500)
  }
</script>

<!-- min-w-0: inside grid/flex, without it a long line widens the page. -->
<div class="group relative min-w-0 rounded-md border bg-muted/40">
  <!-- A horizontally scrolling block needs focus to scroll with the arrow keys
       (WCAG 2.1.1); hence the tabindex on a non-interactive element. -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <pre
    tabindex={wrap ? undefined : 0}
    aria-label={wrap ? undefined : t('connect.code.label')}
    class={[
      'px-4 py-3.5 pr-14 font-mono text-xs leading-relaxed sm:text-[0.8125rem]',
      wrap ? 'break-all whitespace-pre-wrap' : 'overflow-x-auto',
    ]}>{code}</pre>
  <button
    type="button"
    class="absolute top-1.5 right-1.5 grid size-8 cursor-pointer place-items-center rounded-md text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
    aria-label={label ?? t('common.copy')}
    onclick={copy}
  >
    {#if copied}<Check class="size-4 text-brand" />{:else}<Copy class="size-4" />{/if}
  </button>
</div>
