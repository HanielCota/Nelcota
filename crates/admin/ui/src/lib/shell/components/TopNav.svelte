<script lang="ts">
  import * as Sheet from '$lib/components/ui/sheet'
  import Menu from '@lucide/svelte/icons/menu'
  import Search from '@lucide/svelte/icons/search'
  import ProjectSwitcher from '$lib/features/projects/components/ProjectSwitcher.svelte'
  import AccountMenu from './AccountMenu.svelte'
  import { palette } from '$lib/shell/palette.svelte'
  import { href } from '$lib/router.svelte'
  import { isActive, navPills } from '$lib/shell/nav'
  import { t } from '$lib/i18n/index.svelte'

  // The panel's frame (D98), after the user's reference dashboard: three
  // columns, the pages as pills in the middle. Below lg the pills take a row
  // of their own; on phones they live in a sheet.
  let mobileOpen = $state(false)
  const isMac = /Mac|iPhone|iPad/.test(navigator.platform)

  const pill = (active: boolean) => [
    'flex h-10 shrink-0 items-center rounded-full px-4 text-sm whitespace-nowrap transition-colors',
    active ? 'bg-nav-active font-medium text-nav-active-foreground' : 'text-muted-foreground hover:text-foreground',
  ]
</script>

{#snippet search()}
  <!-- Search everything (the command palette), with its shortcut in view so it
       does not read as one more page. -->
  <button
    type="button"
    onclick={() => (palette.open = true)}
    class="flex h-14 shrink-0 cursor-pointer items-center gap-2.5 rounded-full bg-nav px-4 text-sm text-muted-foreground transition-colors hover:bg-accent hover:text-foreground 2xl:pr-2"
    aria-label={`${t('shell.topbar.searchLabel')} (${isMac ? '⌘' : 'Ctrl'} K)`}
  >
    <Search class="size-[18px]" aria-hidden="true" />
    <span class="hidden 2xl:inline">{t('shell.topbar.searchLabel')}</span>
    <kbd class="hidden h-9 items-center rounded-full bg-well px-3 font-sans text-xs 2xl:flex">{isMac ? '⌘' : 'Ctrl'} K</kbd>
  </button>
{/snippet}

{#snippet pills()}
  {#each navPills as item (item.path)}
    {@const active = isActive(item.path, item.also)}
    <a href={href(item.path)} aria-current={active ? 'page' : undefined} class={pill(active)}>{t(item.pill ?? item.title)}</a>
  {/each}
{/snippet}

<header class="shrink-0 bg-background/85 backdrop-blur">
  <div class="mx-auto w-full max-w-page px-4 py-4 sm:px-6 lg:px-8">
  <div class="grid grid-cols-[1fr_auto] items-center gap-3 lg:grid-cols-[1fr_auto_1fr] lg:gap-8">
    <div class="flex min-w-0 items-center gap-2">
      <button
        type="button"
        class="grid size-10 shrink-0 cursor-pointer place-items-center rounded-full bg-nav text-muted-foreground hover:text-foreground md:hidden"
        onclick={() => (mobileOpen = true)}
        aria-label={t('shell.nav.menu')}
      >
        <Menu class="size-5" />
      </button>
      <div class="min-w-0 max-w-64"><ProjectSwitcher /></div>
    </div>

    <nav aria-label={t('shell.nav.main')} class="hidden items-center gap-1 rounded-full bg-nav p-1.5 lg:flex">
      {@render pills()}
    </nav>

    <div class="flex min-w-0 items-center justify-end gap-2">
      {@render search()}
      <AccountMenu />
    </div>
  </div>

  <!-- Below lg: the pills get a row of their own. -->
  <nav aria-label={t('shell.nav.main')} class="mt-3 hidden overflow-x-auto md:block lg:hidden">
    <div class="mx-auto flex w-max items-center gap-1 rounded-full bg-nav p-1.5">{@render pills()}</div>
  </nav>
  </div>
</header>

<Sheet.Root bind:open={mobileOpen}>
  <Sheet.Content side="left" class="w-72 gap-0 p-0">
    <Sheet.Title class="sr-only">{t('shell.nav.main')}</Sheet.Title>
    <div class="flex shrink-0 items-center gap-2 p-3 pr-12">
      <div class="min-w-0 flex-1"><ProjectSwitcher /></div>
    </div>
    <nav class="grid content-start gap-1 overflow-y-auto p-3" aria-label={t('shell.nav.main')}>
      {#each navPills as item (item.path)}
        {@const active = isActive(item.path, item.also)}
        <a href={href(item.path)} onclick={() => (mobileOpen = false)} aria-current={active ? 'page' : undefined} class={[...pill(active), 'gap-3']}>
          <item.icon class="size-[18px]" strokeWidth={1.6} />{t(item.title)}
        </a>
      {/each}
    </nav>
  </Sheet.Content>
</Sheet.Root>
