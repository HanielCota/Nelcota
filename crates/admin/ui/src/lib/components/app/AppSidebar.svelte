<script lang="ts">
  import BookOpen from '@lucide/svelte/icons/book-open'
  import PanelLeftClose from '@lucide/svelte/icons/panel-left-close'
  import PanelLeftOpen from '@lucide/svelte/icons/panel-left-open'
  import ArrowUpRight from '@lucide/svelte/icons/arrow-up-right'
  import { href } from '$lib/router.svelte'
  import { isActive, navGroups } from '$lib/nav'
  import { sidebar, togglePinned } from '$lib/sidebar.svelte'

  // Fixada: largura cheia a partir de xl, com rótulos sempre visíveis.
  // Recolhida (ou telas médias): trilho de ícones que se expande por cima do
  // conteúdo ao passar o mouse ou receber foco.
  const pinned = $derived(sidebar.pinned)
  const reveal = $derived(
    pinned
      ? 'opacity-0 group-hover/rail:opacity-100 group-focus-within/rail:opacity-100 xl:opacity-100'
      : 'opacity-0 group-hover/rail:opacity-100 group-focus-within/rail:opacity-100',
  )
</script>

<div class={['relative z-30 hidden w-16 shrink-0 transition-[width] duration-200 ease-out md:block', pinned && 'xl:w-60']}>
  <nav
    aria-label="Navegação principal"
    class={[
      'group/rail absolute inset-y-0 left-0 flex w-16 flex-col overflow-hidden border-r border-sidebar-border bg-sidebar py-3 transition-[width,box-shadow] duration-200 ease-out',
      'hover:w-60 hover:shadow-overlay focus-within:w-60 focus-within:shadow-overlay',
      pinned && 'xl:w-60 xl:hover:shadow-none xl:focus-within:shadow-none',
    ]}
  >
    <div class="flex flex-1 flex-col gap-4 overflow-x-hidden overflow-y-auto">
      {#each navGroups as group, g (g)}
        <div>
          {#if group.label}
            <p
              class={[
                'mb-1 h-5 px-[23px] text-xs font-medium whitespace-nowrap text-muted-foreground transition-opacity duration-150',
                reveal,
              ]}
            >
              {group.label}
            </p>
          {/if}
          <ul class="flex flex-col gap-0.5 px-3">
            {#each group.items as item (item.path)}
              {@const active = isActive(item.path)}
              <li>
                <a
                  href={href(item.path)}
                  aria-current={active ? 'page' : undefined}
                  title={item.title}
                  class={[
                    'flex h-9 items-center gap-3 overflow-hidden rounded-md px-[11px] text-sm whitespace-nowrap transition-colors',
                    active
                      ? 'bg-sidebar-accent font-medium text-sidebar-accent-foreground'
                      : 'text-sidebar-foreground hover:bg-sidebar-accent/60 hover:text-sidebar-accent-foreground',
                  ]}
                >
                  <item.icon class={['size-[18px] shrink-0', active && 'text-brand']} strokeWidth={1.6} />
                  <span class={['transition-opacity duration-150', reveal]}>{item.title}</span>
                </a>
              </li>
            {/each}
          </ul>
        </div>
      {/each}
    </div>

    <div class="mt-3 grid gap-1 border-t border-sidebar-border px-3 pt-3">
      <a
        href="/rest/v1/"
        target="_blank"
        rel="noopener"
        title="Docs (OpenAPI)"
        class="flex h-9 items-center gap-3 overflow-hidden rounded-md px-[11px] text-sm whitespace-nowrap text-sidebar-foreground transition-colors hover:bg-sidebar-accent/60 hover:text-sidebar-accent-foreground"
      >
        <BookOpen class="size-[18px] shrink-0" strokeWidth={1.6} />
        <span class={['flex flex-1 items-center justify-between transition-opacity duration-150', reveal]}
          >Docs (OpenAPI)<ArrowUpRight class="size-3.5 text-muted-foreground" /></span
        >
      </a>
      <button
        type="button"
        onclick={togglePinned}
        title={pinned ? 'Recolher barra lateral' : 'Fixar barra lateral aberta'}
        aria-label={pinned ? 'Recolher barra lateral' : 'Fixar barra lateral aberta'}
        aria-pressed={pinned}
        class="hidden h-9 cursor-pointer items-center gap-3 overflow-hidden rounded-md px-[11px] text-sm whitespace-nowrap text-sidebar-foreground transition-colors hover:bg-sidebar-accent/60 hover:text-sidebar-accent-foreground xl:flex"
      >
        {#if pinned}
          <PanelLeftClose class="size-[18px] shrink-0" strokeWidth={1.6} />
        {:else}
          <PanelLeftOpen class="size-[18px] shrink-0" strokeWidth={1.6} />
        {/if}
        <span class={['transition-opacity duration-150', reveal]}>{pinned ? 'Recolher' : 'Fixar aberta'}</span>
      </button>
    </div>
  </nav>
</div>
