<script lang="ts">
  import * as Command from '$lib/components/ui/command'
  import { toggleMode } from 'mode-watcher'
  import Table2 from '@lucide/svelte/icons/table-2'
  import FileCode from '@lucide/svelte/icons/file-code'
  import Sparkles from '@lucide/svelte/icons/sparkles'
  import SunMoon from '@lucide/svelte/icons/sun-moon'
  import LogOut from '@lucide/svelte/icons/log-out'
  import BookOpen from '@lucide/svelte/icons/book-open'
  import { api } from '$lib/api'
  import { logout } from '$lib/auth'
  import { navItems } from '$lib/nav'
  import { palette } from '$lib/palette.svelte'
  import { navigate } from '$lib/router.svelte'
  import { commandScore } from '$lib/search'
  import { SQL_SNIPPETS } from '$lib/sql-snippets'
  import { sqlStore } from '$lib/sql-store.svelte'
  import type { TableSummary } from '$lib/types'

  let tables = $state<TableSummary[]>([])

  async function loadTables(signal: AbortSignal) {
    try {
      tables = (await api.get<{ tables: TableSummary[] }>('/tables', { signal })).tables
    } catch {
      // Sem a lista (ou cancelada), a paleta segue com páginas e ações.
    }
  }

  // Carrega já ao montar (quem digita rápido logo ao abrir encontra as tabelas)
  // e atualiza a cada abertura, porque podem ter mudado; enquanto isso, a
  // lista anterior continua visível.
  $effect(() => {
    void palette.open
    const controller = new AbortController()
    loadTables(controller.signal)
    return () => controller.abort()
  })

  function run(action: () => void) {
    palette.open = false
    action()
  }

  function openSql(sql: string, id: string | null = null) {
    sqlStore.open(sql, id)
    navigate('/sql')
  }

  function onKeydown(event: KeyboardEvent) {
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'k') {
      event.preventDefault()
      palette.open = !palette.open
    }
  }

  const pages = navItems

  // Ícone dentro de um quadrado, como nos itens da barra lateral.
  const tile =
    'grid size-8 shrink-0 place-items-center rounded-lg border bg-card text-muted-foreground shadow-card group-data-selected/command-item:border-brand/30 group-data-selected/command-item:bg-brand-soft group-data-selected/command-item:text-brand [&_svg]:size-4'
  // Títulos dos grupos: o componente fixa as classes, então ajustamos daqui.
  const group =
    'px-2 py-1.5 *:first:px-3! *:first:pt-2! *:first:pb-1.5! *:first:text-3xs! *:first:font-semibold! *:first:tracking-[0.08em]! *:first:uppercase'
  const kbd =
    'inline-grid min-w-5 place-items-center rounded-md border border-border-strong bg-card px-1.5 py-0.5 font-sans text-2xs font-medium text-foreground shadow-card'
</script>

<svelte:window onkeydown={onKeydown} />

<Command.Dialog
  bind:open={palette.open}
  filter={commandScore}
  title="Paleta de comandos"
  description="Busque páginas, tabelas, consultas e ações"
  class="gap-0 sm:max-w-xl"
>
  <Command.Input placeholder="Buscar páginas, tabelas, consultas…" />
  <Command.List class="max-h-[min(60vh,460px)] p-1.5">
    <Command.Empty class="py-10 text-muted-foreground">Nada encontrado.</Command.Empty>

    <Command.Group heading="Páginas" class={group}>
      {#each pages as page (page.path)}
        <Command.Item value={`página ${page.title}`} onSelect={() => run(() => navigate(page.path))}>
          <span class={tile}><page.icon /></span>{page.title}
        </Command.Item>
      {/each}
    </Command.Group>

    {#if tables.length}
      <Command.Group heading="Tabelas" class={group}>
        {#each tables as table (table.name)}
          <Command.Item
            value={`tabela ${table.name}`}
            onSelect={() => run(() => navigate(`/tables/${encodeURIComponent(table.name)}`))}
          >
            <span class={tile}><Table2 /></span><span class="truncate font-mono text-xs">{table.name}</span>
            {#if table.kind !== 'table'}<Command.Shortcut>view</Command.Shortcut>{/if}
          </Command.Item>
        {/each}
      </Command.Group>
    {/if}

    {#if sqlStore.saved.length}
      <Command.Group heading="Consultas salvas" class={group}>
        {#each sqlStore.sorted as query (query.id)}
          <Command.Item value={`consulta ${query.name} ${query.id}`} onSelect={() => run(() => openSql(query.sql, query.id))}>
            <span class={tile}><FileCode /></span><span class="truncate">{query.name}</span>
          </Command.Item>
        {/each}
      </Command.Group>
    {/if}

    <Command.Group heading="Modelos SQL" class={group}>
      {#each SQL_SNIPPETS as snippet (snippet.label)}
        <Command.Item value={`modelo ${snippet.label}`} onSelect={() => run(() => openSql(snippet.sql))}>
          <span class={tile}><Sparkles /></span>{snippet.label}
        </Command.Item>
      {/each}
    </Command.Group>

    <Command.Separator />
    <Command.Group heading="Ações" class={group}>
      <Command.Item value="ação alternar tema claro escuro" onSelect={() => run(toggleMode)}>
        <span class={tile}><SunMoon /></span>Alternar tema
      </Command.Item>
      <Command.Item value="ação documentação api openapi" onSelect={() => run(() => window.open('/rest/v1/', '_blank', 'noopener'))}>
        <span class={tile}><BookOpen /></span>Abrir documentação da API
      </Command.Item>
      <Command.Item value="ação sair logout" onSelect={() => run(logout)}>
        <span class={tile}><LogOut /></span>Sair
      </Command.Item>
    </Command.Group>
  </Command.List>
  <div class="-mx-1 -mb-1 hidden items-center gap-4 border-t bg-muted/40 px-4 py-2.5 text-xs text-muted-foreground sm:flex" aria-hidden="true">
    <span class="flex items-center gap-1.5"><kbd class={kbd}>↑</kbd><kbd class={kbd}>↓</kbd>navegar</span>
    <span class="flex items-center gap-1.5"><kbd class={kbd}>↵</kbd>abrir</span>
    <span class="ml-auto flex items-center gap-1.5"><kbd class={kbd}>Esc</kbd>fechar</span>
  </div>
</Command.Dialog>
