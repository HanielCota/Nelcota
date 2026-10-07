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
</script>

<svelte:window onkeydown={onKeydown} />

<Command.Dialog
  bind:open={palette.open}
  filter={commandScore}
  title="Paleta de comandos"
  description="Busque páginas, tabelas, consultas e ações"
  class="sm:max-w-xl"
>
  <Command.Input placeholder="Buscar páginas, tabelas, consultas…" />
  <Command.List class="max-h-[min(60vh,420px)]">
    <Command.Empty class="py-8 text-muted-foreground">Nada encontrado.</Command.Empty>

    <Command.Group heading="Páginas">
      {#each pages as page (page.path)}
        <Command.Item value={`página ${page.title}`} onSelect={() => run(() => navigate(page.path))}>
          <page.icon />{page.title}
        </Command.Item>
      {/each}
    </Command.Group>

    {#if tables.length}
      <Command.Group heading="Tabelas">
        {#each tables as table (table.name)}
          <Command.Item
            value={`tabela ${table.name}`}
            onSelect={() => run(() => navigate(`/tables/${encodeURIComponent(table.name)}`))}
          >
            <Table2 /><span class="truncate">{table.name}</span>
            {#if table.kind !== 'table'}<Command.Shortcut>view</Command.Shortcut>{/if}
          </Command.Item>
        {/each}
      </Command.Group>
    {/if}

    {#if sqlStore.saved.length}
      <Command.Group heading="Consultas salvas">
        {#each sqlStore.sorted as query (query.id)}
          <Command.Item value={`consulta ${query.name} ${query.id}`} onSelect={() => run(() => openSql(query.sql, query.id))}>
            <FileCode /><span class="truncate">{query.name}</span>
          </Command.Item>
        {/each}
      </Command.Group>
    {/if}

    <Command.Group heading="Modelos SQL">
      {#each SQL_SNIPPETS as snippet (snippet.label)}
        <Command.Item value={`modelo ${snippet.label}`} onSelect={() => run(() => openSql(snippet.sql))}>
          <Sparkles />{snippet.label}
        </Command.Item>
      {/each}
    </Command.Group>

    <Command.Separator />
    <Command.Group heading="Ações">
      <Command.Item value="ação alternar tema claro escuro" onSelect={() => run(toggleMode)}>
        <SunMoon />Alternar tema
      </Command.Item>
      <Command.Item value="ação documentação api openapi" onSelect={() => run(() => window.open('/rest/v1/', '_blank', 'noopener'))}>
        <BookOpen />Abrir documentação da API
      </Command.Item>
      <Command.Item value="ação sair logout" onSelect={() => run(logout)}>
        <LogOut />Sair
      </Command.Item>
    </Command.Group>
  </Command.List>
</Command.Dialog>
