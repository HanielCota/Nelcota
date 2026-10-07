<script lang="ts">
  import * as Sheet from '$lib/components/ui/sheet'
  import { Button } from '$lib/components/ui/button'
  import Menu from '@lucide/svelte/icons/menu'
  import Plug from '@lucide/svelte/icons/plug'
  import Search from '@lucide/svelte/icons/search'
  import Logo from './Logo.svelte'
  import ProjectSwitcher from './ProjectSwitcher.svelte'
  import AccountMenu from './AccountMenu.svelte'
  import { palette } from '$lib/palette.svelte'
  import { t } from '$lib/i18n/index.svelte'
  import { href, route } from '$lib/router.svelte'
  import { isActive, navGroups } from '$lib/nav'
  import { crumbsFor } from '$lib/titles'

  // Top bar of the content area: breadcrumb and search. Project, pages and
  // account live in the sidebar (on phones, in the menu that slides in).

  const crumbs = $derived(crumbsFor(route.path))

  let mobileOpen = $state(false)
  const isMac = /Mac|iPhone|iPad/.test(navigator.platform)
</script>

<header class="flex h-14 shrink-0 items-center gap-2 border-b border-sidebar-border bg-sidebar px-3 sm:px-4">
  <div class="flex shrink-0 items-center gap-1 md:hidden">
    <Button variant="ghost" size="icon" onclick={() => (mobileOpen = true)} aria-label={t('shell.nav.menu')}>
      <Menu class="size-5" />
    </Button>
    <a href={href('/')} aria-label={t('shell.pages.overview')}><Logo mark /></a>
  </div>

  <nav class="flex min-w-0 items-center gap-1.5 text-sm font-medium" aria-label={t('shell.nav.breadcrumb')}>
    {#each crumbs as crumb, i (i)}
      {@const last = i === crumbs.length - 1}
      <!-- On phones only the current page fits without truncation. -->
      <span class={['min-w-0 items-center gap-1.5', last ? 'flex' : 'hidden sm:flex']}>
        {#if i > 0}
          <svg viewBox="0 0 24 24" class="hidden size-4 shrink-0 text-border-strong sm:block" aria-hidden="true">
            <path d="M16 3 8 21" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" fill="none" />
          </svg>
        {/if}
        {#if crumb.path}
          <a
            href={href(crumb.path)}
            class="truncate rounded-md px-1 text-muted-foreground transition-colors hover:text-foreground"
            >{crumb.label}</a
          >
        {:else}
          <span class="truncate px-1 text-foreground" aria-current="page">{crumb.label}</span>
        {/if}
      </span>
    {/each}
  </nav>

  <div class="ml-auto flex items-center gap-2">
    <button
      type="button"
      onclick={() => (palette.open = true)}
      class="flex size-9 cursor-pointer items-center justify-center gap-2.5 rounded-md border border-border-strong bg-card text-sm text-muted-foreground transition-colors hover:border-ring hover:text-foreground sm:w-64 sm:justify-start sm:px-3 lg:w-80"
      aria-label={t('shell.topbar.searchLabel')}
    >
      <Search class="size-4 shrink-0" />
      <span class="hidden flex-1 text-left sm:inline">{t('shell.topbar.search')}</span>
      <kbd class="hidden rounded border border-border-strong bg-muted px-1.5 font-sans text-2xs sm:inline"
        >{isMac ? '⌘' : 'Ctrl'} K</kbd
      >
    </button>
    <Button variant="outline" href={href('/connect')} class="hidden lg:inline-flex">
      <Plug />{t('shell.topbar.connect')}
    </Button>
  </div>
</header>

<!-- Navigation on small screens, where the sidebar is hidden. -->
<Sheet.Root bind:open={mobileOpen}>
  <Sheet.Content side="left" class="w-72 gap-0 bg-sidebar p-0">
    <div class="flex h-14 shrink-0 items-center gap-2 border-b border-sidebar-border px-4">
      <Logo mark />
      <div class="min-w-0 flex-1 pr-8"><ProjectSwitcher /></div>
    </div>
    <nav class="grid flex-1 content-start gap-4 overflow-y-auto p-3" aria-label={t('shell.nav.main')}>
      {#each navGroups as group, g (g)}
        <div>
          {#if group.label}
            <p class="mb-1 px-3 text-xs font-medium text-muted-foreground">{t(group.label)}</p>
          {/if}
          {#each group.items as item (item.path)}
            {@const active = isActive(item.path)}
            <a
              href={href(item.path)}
              onclick={() => (mobileOpen = false)}
              aria-current={active ? 'page' : undefined}
              class={[
                'flex h-10 items-center gap-3 rounded-md px-3 text-sm',
                active ? 'bg-sidebar-accent font-medium text-foreground' : 'text-sidebar-foreground hover:bg-sidebar-accent/60',
              ]}
            >
              <item.icon class={['size-[18px]', active && 'text-brand']} strokeWidth={1.6} />{t(item.title)}
            </a>
          {/each}
        </div>
      {/each}
    </nav>
    <div class="shrink-0 border-t border-sidebar-border p-3"><AccountMenu /></div>
  </Sheet.Content>
</Sheet.Root>
