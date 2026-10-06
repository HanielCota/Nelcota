<script lang="ts">
  import { onMount } from 'svelte'
  import * as Table from '$lib/components/ui/table'
  import PageHeader from '$lib/components/app/PageHeader.svelte'
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
  const plural = (n: number, one: string, many: string) => `${fmt.format(n)} ${n === 1 ? one : many}`
</script>

<div class="mx-auto max-w-5xl p-6">
  <PageHeader title="Visão geral" />

  {#if error}
    <p class="text-sm text-destructive">{error}</p>
  {:else if !data}
    <p class="text-sm text-muted-foreground">Carregando…</p>
  {:else}
    <p class="mb-5 text-sm text-muted-foreground">
      Schema <span class="font-mono text-foreground">{data.schema}</span>:
      {plural(data.counts.tables, 'tabela', 'tabelas')},
      {plural(data.counts.users, 'usuário', 'usuários')},
      {plural(data.counts.policies, 'policy', 'policies')},
      {plural(data.counts.functions, 'função', 'funções')}.
    </p>

    {#if data.exposed_without_rls.length}
      <div class="mb-5 rounded border border-destructive/40 px-4 py-3 text-sm">
        <p class="font-medium text-destructive">
          Expostas sem RLS: {data.exposed_without_rls.join(', ')}
        </p>
        <p class="mt-1 text-muted-foreground">
          Quem tem GRANT nessas tabelas lê e altera todas as linhas. Ative com
          <span class="font-mono">alter table … enable row level security</span> e crie policies, ou revogue
          os grants.
        </p>
      </div>
    {/if}

    {#if data.tables.length === 0}
      <p class="text-sm text-muted-foreground">
        Nenhuma tabela. Crie com <span class="font-mono">nelcota migrate</span> ou na aba
        <a href={href('/sql')} class="underline underline-offset-2">SQL</a>.
      </p>
    {:else}
      <div class="rounded border">
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
                  <a href={href(`/tables/${encodeURIComponent(table.name)}`)} class="font-medium hover:underline">
                    {table.name}
                  </a>
                  {#if table.kind !== 'table'}<span class="ml-1 text-xs text-muted-foreground">view</span>{/if}
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
      </div>
    {/if}
  {/if}
</div>
