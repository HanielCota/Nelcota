<script lang="ts">
  import { href } from '$lib/router.svelte'

  // Sibling pages share a simple tab line.
  let {
    tabs,
    current,
    label,
    fill = false,
  }: {
    tabs: { path: string; label: string }[]
    current: string
    label: string
    /** Spread the tabs over the whole width (in a sidebar). */
    fill?: boolean
  } = $props()
</script>

<nav aria-label={label} class={['flex max-w-full items-center gap-3 overflow-x-auto border-b', fill ? 'w-full' : 'w-max']}>
  {#each tabs as tab (tab.path)}
    {@const active = tab.path === current}
    <a
      href={href(tab.path)}
      aria-current={active ? 'page' : undefined}
      class={[
        'flex h-10 items-center border-b-2 text-sm whitespace-nowrap transition-colors',
        fill ? 'flex-1 justify-center px-2' : 'shrink-0 px-1',
        active ? 'border-foreground font-medium text-foreground' : 'border-transparent text-muted-foreground hover:border-border-strong hover:text-foreground',
      ]}>{tab.label}</a
    >
  {/each}
</nav>
