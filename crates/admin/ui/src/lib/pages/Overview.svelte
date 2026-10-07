<script lang="ts">
  import { onMount } from 'svelte'
  import * as Table from '$lib/components/ui/table'
  import { Skeleton } from '$lib/components/ui/skeleton'
  import { Button } from '$lib/components/ui/button'
  import PageHeader from '$lib/components/app/PageHeader.svelte'
  import EmptyState from '$lib/components/app/EmptyState.svelte'
  import Callout from '$lib/components/app/Callout.svelte'
  import RlsBadge from '$lib/components/app/RlsBadge.svelte'
  import Grants from '$lib/components/app/Grants.svelte'
  import { api } from '$lib/api'
  import { href } from '$lib/router.svelte'
  import type { Overview } from '$lib/types'

  let data = $state<Overview | null>(null)
  let error = $state('')

  onMount(async () => {
    try {
      data = await api.get<Overview>('/overview')
    } catch (e) {
      error = (e as Error).message
    }
  })

  const fmt = new Intl.NumberFormat('pt-BR')

  const stats = $derived(
    data
      ? [
          { label: 'Tabelas', value: data.counts.tables, path: '/tables' },
          { label: 'Usuários', value: data.counts.users, path: '/users' },
          { label: 'Policies', value: data.counts.policies, path: '/policies' },
          { label: 'Funções', value: data.counts.functions, path: '/sql' },
        ]
      : [],
  )
</script>

<div class="mx-auto max-w-6xl px-4 py-8 sm:px-6 lg:px-10 lg:py-10">
  <PageHeader
    title="Visão geral"
    description={data ? `Schema ${data.schema} exposto pela API REST.` : undefined}
  />

  {#if error}
    <p class="text-sm text-destructive">{error}</p>
  {:else if !data}
    <div class="grid grid-cols-2 gap-3 lg:grid-cols-4">
      {#each [0, 1, 2, 3] as i (i)}<Skeleton class="h-24 rounded-lg" />{/each}
    </div>
    <Skeleton class="mt-10 h-64 rounded-lg" />
  {:else}
    <div class="grid grid-cols-2 gap-3 lg:grid-cols-4">
      {#each stats as stat (stat.label)}
        <a
          href={href(stat.path)}
          class="rounded-lg border bg-card px-5 py-4 transition-colors hover:border-border-strong hover:bg-muted/40"
        >
          <p class="text-sm text-muted-foreground">{stat.label}</p>
          <p class="mt-2 text-2xl font-semibold tabular-nums">{fmt.format(stat.value)}</p>
        </a>
      {/each}
    </div>

    {#if data.exposed_without_rls.length}
      <Callout variant="danger" title={`Expostas sem RLS: ${data.exposed_without_rls.join(', ')}`} class="mt-6">
        Quem tem GRANT nessas tabelas lê e altera todas as linhas. Ative com
        <code class="text-xs text-foreground">alter table … enable row level security</code> e crie policies, ou
        revogue os grants.
      </Callout>
    {/if}

    <section class="mt-10">
      <div class="mb-3 flex items-center justify-between gap-3">
        <h2 class="text-base font-semibold">Tabelas</h2>
        <Button variant="ghost" size="sm" href={href('/tables')}>Abrir editor</Button>
      </div>

      <div class="overflow-hidden rounded-lg border bg-card">
        {#if data.tables.length === 0}
          <EmptyState title="Nenhuma tabela ainda">
            Crie com <code class="text-xs text-foreground">nelcota migrate</code> ou no
            <a href={href('/sql')} class="text-brand hover:underline">Editor SQL</a>.
          </EmptyState>
        {:else}
          <Table.Root>
            <Table.Header>
              <Table.Row class="hover:bg-transparent">
                <Table.Head>Tabela</Table.Head>
                <Table.Head class="text-right">Linhas</Table.Head>
                <Table.Head>RLS</Table.Head>
                <Table.Head>anon</Table.Head>
                <Table.Head>authenticated</Table.Head>
              </Table.Row>
            </Table.Header>
            <Table.Body>
              {#each data.tables as table (table.name)}
                <Table.Row>
                  <Table.Cell>
                    <a href={href(`/tables/${encodeURIComponent(table.name)}`)} class="font-medium hover:text-brand"
                      >{table.name}</a
                    >
                    {#if table.kind !== 'table'}<span class="ml-1.5 text-xs text-muted-foreground">view</span>{/if}
                  </Table.Cell>
                  <Table.Cell class="text-right font-mono text-xs tabular-nums">
                    {table.rows === null ? '—' : `${table.rows_exact ? '' : '~'}${fmt.format(table.rows)}`}
                  </Table.Cell>
                  <Table.Cell><RlsBadge rls={table.rls} /></Table.Cell>
                  <Table.Cell><Grants grants={table.grants.anon} /></Table.Cell>
                  <Table.Cell><Grants grants={table.grants.authenticated} /></Table.Cell>
                </Table.Row>
              {/each}
            </Table.Body>
          </Table.Root>
        {/if}
      </div>
    </section>
  {/if}
</div>
