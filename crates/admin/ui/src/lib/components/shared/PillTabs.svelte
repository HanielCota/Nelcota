<script lang="ts">
  import { href } from '$lib/router.svelte'

  // Sibling pages as pills (D98), e.g. Users and User sign-in.
  let { tabs, current, label }: { tabs: { path: string; label: string }[]; current: string; label: string } = $props()
</script>

<nav aria-label={label} class="flex w-max max-w-full items-center gap-1 overflow-x-auto rounded-full bg-card p-1.5">
  {#each tabs as tab (tab.path)}
    {@const active = tab.path === current}
    <a
      href={href(tab.path)}
      aria-current={active ? 'page' : undefined}
      class={[
        'flex h-9 shrink-0 items-center rounded-full px-4 text-sm whitespace-nowrap transition-colors',
        active ? 'bg-nav-active font-medium text-nav-active-foreground' : 'text-muted-foreground hover:bg-accent hover:text-foreground',
      ]}>{tab.label}</a
    >
  {/each}
</nav>
