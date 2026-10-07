<script lang="ts">
  import BookOpen from '@lucide/svelte/icons/book-open'
  import { href } from '$lib/router.svelte'
  import { isActive, navGroups } from '$lib/nav'

  // Trilho de ícones que se expande por cima do conteúdo ao passar o mouse.
</script>

<div class="relative z-30 hidden w-14 shrink-0 md:block">
  <nav
    aria-label="Navegação principal"
    class="group/rail absolute inset-y-0 left-0 flex w-14 flex-col overflow-hidden border-r border-sidebar-border bg-sidebar py-2 transition-[width,box-shadow] duration-200 ease-out hover:w-52 hover:shadow-2xl hover:shadow-black/30 focus-within:w-52 focus-within:shadow-2xl focus-within:shadow-black/30"
  >
    {#each navGroups as group, g (g)}
      {#if g > 0}<div class="mx-3 my-2 border-t border-sidebar-border"></div>{/if}
      <ul class="flex flex-col gap-0.5 px-2">
        {#each group as item (item.path)}
          {@const active = isActive(item.path)}
          <li>
            <a
              href={href(item.path)}
              aria-current={active ? 'page' : undefined}
              class={[
                'flex h-9 items-center gap-3 overflow-hidden rounded-md px-[9px] text-sm whitespace-nowrap transition-colors',
                active
                  ? 'bg-sidebar-accent text-sidebar-accent-foreground'
                  : 'text-sidebar-foreground hover:bg-sidebar-accent/60 hover:text-sidebar-accent-foreground',
              ]}
            >
              <item.icon class={['size-[18px] shrink-0', active && 'text-brand']} strokeWidth={1.6} />
              <span class="opacity-0 transition-opacity duration-150 group-hover/rail:opacity-100 group-focus-within/rail:opacity-100">{item.title}</span>
            </a>
          </li>
        {/each}
      </ul>
    {/each}

    <div class="mt-auto px-2">
      <a
        href="/rest/v1/"
        target="_blank"
        rel="noopener"
        class="flex h-9 items-center gap-3 overflow-hidden rounded-md px-[9px] text-sm whitespace-nowrap text-sidebar-foreground transition-colors hover:bg-sidebar-accent/60 hover:text-sidebar-accent-foreground"
      >
        <BookOpen class="size-[18px] shrink-0" strokeWidth={1.6} />
        <span class="opacity-0 transition-opacity duration-150 group-hover/rail:opacity-100 group-focus-within/rail:opacity-100">Docs (OpenAPI)</span>
      </a>
      <p
        class="mt-2 w-48 px-2 pb-1 text-2xs leading-snug font-light text-muted-foreground opacity-0 transition-opacity duration-150 group-hover/rail:opacity-100 group-focus-within/rail:opacity-100"
      >
        A chave <span class="font-mono">service_role</span> ignora o RLS. Use só no backend.
      </p>
    </div>
  </nav>
</div>
