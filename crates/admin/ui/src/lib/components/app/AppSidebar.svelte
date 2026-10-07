<script lang="ts">
  import BookOpen from '@lucide/svelte/icons/book-open'
  import PanelLeftClose from '@lucide/svelte/icons/panel-left-close'
  import PanelLeftOpen from '@lucide/svelte/icons/panel-left-open'
  import ArrowUpRight from '@lucide/svelte/icons/arrow-up-right'
  import Logo from './Logo.svelte'
  import ProjectSwitcher from './ProjectSwitcher.svelte'
  import AccountMenu from './AccountMenu.svelte'
  import { href } from '$lib/router.svelte'
  import { isActive, navGroups } from '$lib/nav'
  import { sidebar, togglePinned } from '$lib/sidebar.svelte'
  import { t } from '$lib/i18n/index.svelte'

  // Full-height column: project at the top, pages in the middle, account at
  // the bottom. Pinned: full width from xl up, labels always visible.
  // Collapsed (or medium screens): an icon rail that expands over the
  // content on hover or focus.
  const pinned = $derived(sidebar.pinned)

  // While a menu (project, account) is open the rail stays expanded: the
  // menu renders outside it and would otherwise collapse it while in use.
  let projectOpen = $state(false)
  let accountOpen = $state(false)
  const held = $derived(projectOpen || accountOpen)

  const reveal = $derived(
    held
      ? 'opacity-100'
      : [
          'opacity-0 group-hover/rail:opacity-100 group-focus-within/rail:opacity-100',
          pinned && 'xl:opacity-100',
        ].join(' '),
  )
</script>

<div class={['relative z-30 hidden w-16 shrink-0 transition-[width] duration-200 ease-out md:block', pinned && 'xl:w-60']}>
  <aside
    class={[
      'group/rail absolute inset-y-0 left-0 flex w-16 flex-col overflow-hidden border-r border-sidebar-border bg-sidebar transition-[width,box-shadow] duration-200 ease-out',
      'hover:w-60 hover:shadow-overlay focus-within:w-60 focus-within:shadow-overlay',
      held && 'w-60 shadow-overlay',
      pinned && 'xl:w-60 xl:shadow-none xl:hover:shadow-none xl:focus-within:shadow-none',
    ]}
  >
    <!-- Same height as the topbar: the bottom border continues it. -->
    <div class="flex h-14 shrink-0 items-center gap-1 border-b border-sidebar-border pr-3">
      <a href={href('/')} class="grid w-16 shrink-0 place-items-center" aria-label={t('shell.pages.overview')}><Logo mark size="lg" /></a>
      <div class={['min-w-0 flex-1 transition-opacity duration-150', reveal]}>
        <ProjectSwitcher bind:menuOpen={projectOpen} />
      </div>
    </div>

    <nav aria-label={t('shell.nav.main')} class="flex flex-1 flex-col gap-4 overflow-x-hidden overflow-y-auto py-3">
      {#each navGroups as group, g (g)}
        <div>
          {#if group.label}
            <p
              class={[
                'mb-1 h-5 px-[23px] text-xs font-medium whitespace-nowrap text-muted-foreground transition-opacity duration-150',
                reveal,
              ]}
            >
              {t(group.label)}
            </p>
          {/if}
          <ul class="flex flex-col gap-0.5 px-3">
            {#each group.items as item (item.path)}
              {@const active = isActive(item.path)}
              <li>
                <a
                  href={href(item.path)}
                  aria-current={active ? 'page' : undefined}
                  title={t(item.title)}
                  class={[
                    'flex h-9 items-center gap-3 overflow-hidden rounded-md px-[11px] text-sm whitespace-nowrap transition-colors',
                    active
                      ? 'bg-sidebar-accent font-medium text-sidebar-accent-foreground'
                      : 'text-sidebar-foreground hover:bg-sidebar-accent/60 hover:text-sidebar-accent-foreground',
                  ]}
                >
                  <item.icon class={['size-[18px] shrink-0', active && 'text-brand']} strokeWidth={1.6} />
                  <span class={['transition-opacity duration-150', reveal]}>{t(item.title)}</span>
                </a>
              </li>
            {/each}
          </ul>
        </div>
      {/each}
    </nav>

    <div class="grid shrink-0 gap-1 border-t border-sidebar-border px-3 py-3">
      <a
        href="/rest/v1/"
        target="_blank"
        rel="noopener"
        title={t('shell.nav.docs')}
        class="flex h-9 items-center gap-3 overflow-hidden rounded-md px-[11px] text-sm whitespace-nowrap text-sidebar-foreground transition-colors hover:bg-sidebar-accent/60 hover:text-sidebar-accent-foreground"
      >
        <BookOpen class="size-[18px] shrink-0" strokeWidth={1.6} />
        <span class={['flex flex-1 items-center justify-between transition-opacity duration-150', reveal]}
          >{t('shell.nav.docs')}<ArrowUpRight class="size-3.5 text-muted-foreground" /></span
        >
      </a>
      <!-- Width of the open sidebar: collapsed, it only clips the right side (the avatar stays). -->
      <div class="flex w-[216px] items-center gap-1">
        <div class="min-w-0 flex-1"><AccountMenu bind:open={accountOpen} {reveal} /></div>
        <!-- An interface control, not a page: icon only, next to the account. -->
        <button
          type="button"
          onclick={(e) => {
            togglePinned()
            // Focus on the button would keep the rail open (focus-within) and the
            // click would seem to do nothing: release focus when collapsing.
            if (!sidebar.pinned) e.currentTarget.blur()
          }}
          title={pinned ? t('shell.nav.collapse') : t('shell.nav.pin')}
          aria-label={pinned ? t('shell.nav.collapse') : t('shell.nav.pin')}
          aria-pressed={pinned}
          class={[
            'hidden size-8 shrink-0 cursor-pointer place-items-center rounded-md text-muted-foreground transition-[color,background-color,opacity] duration-150 hover:bg-sidebar-accent/60 hover:text-foreground xl:grid',
            reveal,
          ]}
        >
          {#if pinned}<PanelLeftClose class="size-4" />{:else}<PanelLeftOpen class="size-4" />{/if}
        </button>
      </div>
    </div>
  </aside>
</div>
