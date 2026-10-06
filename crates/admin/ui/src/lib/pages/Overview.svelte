<script lang="ts">
  import { onMount } from 'svelte'
  import * as Table from '$lib/components/ui/table'
  import { Skeleton } from '$lib/components/ui/skeleton'
  import Table2 from '@lucide/svelte/icons/table-2'
  import Users from '@lucide/svelte/icons/users'
  import ShieldCheck from '@lucide/svelte/icons/shield-check'
  import SquareFunction from '@lucide/svelte/icons/square-function'
  import TriangleAlert from '@lucide/svelte/icons/triangle-alert'
  import ArrowRight from '@lucide/svelte/icons/arrow-right'
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

  const stats = $derived(
    data
      ? [
          { label: 'Tabelas', value: data.counts.tables, icon: Table2, path: '/tables' },
          { label: 'Usuários', value: data.counts.users, icon: Users, path: '/users' },
          { label: 'Policies', value: data.counts.policies, icon: ShieldCheck, path: '/policies' },
          { label: 'Funções', value: data.counts.functions, icon: SquareFunction, path: '/sql' },
        ]
      : [],
  )
</script>

<div class="mx-auto max-w-6xl px-6 py-10 lg:px-10">
  <PageHeader
    title="Visão geral"
    description={data ? `Schema ${data.schema} exposto pela API REST.` : 'Resumo do banco deste projeto.'}
  />

  {#if error}
    <p class="rounded-md border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{error}</p>
  {:else if !data}
    <div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
      {#each [0, 1, 2, 3] as i (i)}<Skeleton class="h-28 rounded-lg" />{/each}
    </div>
    <Skeleton class="mt-8 h-64 rounded-lg" />
  {:else}
    <div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
      {#each stats as stat (stat.label)}
        <a
          href={href(stat.path)}
          class="group rounded-lg border bg-card p-5 transition-colors hover:border-border-strong hover:bg-muted/40"
        >
          <div class="flex items-center justify-between text-muted-foreground">
            <span class="text-sm">{stat.label}</span>
            <stat.icon class="size-4 transition-colors group-hover:text-brand" strokeWidth={1.6} />
          </div>
          <p class="mt-4 text-3xl font-light tracking-tight tabular-nums">{fmt.format(stat.value)}</p>
        </a>
      {/each}
    </div>

    {#if data.exposed_without_rls.length}
      <div class="mt-6 flex gap-3 rounded-lg border border-destructive/30 bg-destructive/5 p-4 text-sm">
        <TriangleAlert class="mt-0.5 size-4 shrink-0 text-destructive" />
        <div>
          <p class="font-medium text-destructive">
            Expostas sem RLS: {data.exposed_without_rls.join(', ')}
          </p>
          <p class="mt-1 font-light text-muted-foreground">
            Quem tem GRANT nessas tabelas lê e altera todas as linhas. Ative com
            <code class="text-xs text-foreground">alter table … enable row level security</code> e crie policies,
            ou revogue os grants.
          </p>
        </div>
      </div>
    {/if}

    <section class="mt-10">
      <div class="mb-3 flex items-center justify-between">
        <h2 class="text-base font-medium">Tabelas</h2>
        <a
          href={href('/tables')}
          class="inline-flex items-center gap-1 text-sm text-muted-foreground transition-colors hover:text-foreground"
        >
          Abrir editor <ArrowRight class="size-3.5" />
        </a>
      </div>

      {#if data.tables.length === 0}
        <div class="rounded-lg border border-dashed px-6 py-12 text-center">
          <Table2 class="mx-auto size-6 text-muted-foreground" strokeWidth={1.4} />
          <p class="mt-3 text-sm font-medium">Nenhuma tabela ainda</p>
          <p class="mt-1 text-sm font-light text-muted-foreground">
            Crie com <code class="text-xs text-foreground">nelcota migrate</code> ou no
            <a href={href('/sql')} class="text-brand hover:underline">Editor SQL</a>.
          </p>
        </div>
      {:else}
        <div class="overflow-hidden rounded-lg border bg-card">
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
                    <a
                      href={href(`/tables/${encodeURIComponent(table.name)}`)}
                      class="inline-flex items-center gap-2 font-medium hover:text-brand"
                    >
                      <Table2 class="size-3.5 text-muted-foreground" strokeWidth={1.6} />
                      {table.name}
                    </a>
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
        </div>
      {/if}
    </section>
  {/if}
</div>
