<script lang="ts">
  import { onMount } from 'svelte'
  import * as Table from '$lib/components/ui/table'
  import { Skeleton } from '$lib/components/ui/skeleton'
  import { Button } from '$lib/components/ui/button'
  import Table2 from '@lucide/svelte/icons/table-2'
  import Users from '@lucide/svelte/icons/users'
  import ShieldCheck from '@lucide/svelte/icons/shield-check'
  import SquareFunction from '@lucide/svelte/icons/square-function'
  import ArrowRight from '@lucide/svelte/icons/arrow-right'
  import ArrowUpRight from '@lucide/svelte/icons/arrow-up-right'
  import House from '@lucide/svelte/icons/house'
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
          { label: 'Tabelas', hint: 'no schema exposto', value: data.counts.tables, icon: Table2, path: '/tables' },
          { label: 'Usuários', hint: 'em auth.users', value: data.counts.users, icon: Users, path: '/users' },
          { label: 'Policies', hint: 'regras de RLS', value: data.counts.policies, icon: ShieldCheck, path: '/policies' },
          { label: 'Funções', hint: 'chamáveis via rpc', value: data.counts.functions, icon: SquareFunction, path: '/sql' },
        ]
      : [],
  )
</script>

<div class="mx-auto max-w-6xl px-4 py-8 sm:px-6 lg:px-10 lg:py-12">
  <PageHeader
    title="Visão geral"
    icon={House}
    description={data ? `Schema ${data.schema} exposto pela API REST.` : 'Resumo do banco deste projeto.'}
  />

  {#if error}
    <Callout variant="danger" title="Não foi possível carregar o resumo">{error}</Callout>
  {:else if !data}
    <div class="grid grid-cols-2 gap-4 lg:grid-cols-4">
      {#each [0, 1, 2, 3] as i (i)}<Skeleton class="h-36 rounded-xl" />{/each}
    </div>
    <Skeleton class="mt-10 h-72 rounded-xl" />
  {:else}
    <div class="grid grid-cols-2 gap-4 lg:grid-cols-4">
      {#each stats as stat (stat.label)}
        <a
          href={href(stat.path)}
          class="group relative flex flex-col rounded-xl border bg-card p-5 shadow-card transition-[border-color,box-shadow,transform] duration-200 hover:-translate-y-0.5 hover:border-border-strong hover:shadow-raised sm:p-6"
        >
          <div class="flex items-start justify-between gap-3">
            <span
              class="grid size-10 place-items-center rounded-lg border border-brand/20 bg-brand-soft text-brand transition-transform group-hover:scale-105"
            >
              <stat.icon class="size-5" strokeWidth={1.75} />
            </span>
            <ArrowUpRight
              class="size-4 text-muted-foreground opacity-0 transition-all group-hover:translate-x-0.5 group-hover:-translate-y-0.5 group-hover:opacity-100"
            />
          </div>
          <p class="mt-5 text-3xl font-bold tracking-tight tabular-nums sm:text-4xl">{fmt.format(stat.value)}</p>
          <p class="mt-1 text-sm font-semibold">{stat.label}</p>
          <p class="text-xs text-muted-foreground">{stat.hint}</p>
        </a>
      {/each}
    </div>

    {#if data.exposed_without_rls.length}
      <Callout variant="danger" title={`Expostas sem RLS: ${data.exposed_without_rls.join(', ')}`} class="mt-6">
        Quem tem GRANT nessas tabelas lê e altera todas as linhas. Ative com
        <code class="rounded bg-muted px-1 py-0.5 text-xs text-foreground">alter table … enable row level security</code> e
        crie policies, ou revogue os grants.
      </Callout>
    {/if}

    <section class="mt-12">
      <div class="mb-4 flex flex-wrap items-end justify-between gap-3">
        <div>
          <h2 class="text-lg font-semibold tracking-tight">Tabelas</h2>
          <p class="text-sm text-muted-foreground">Linhas, RLS e permissões de cada tabela exposta.</p>
        </div>
        <Button variant="outline" href={href('/tables')}>Abrir editor<ArrowRight /></Button>
      </div>

      {#if data.tables.length === 0}
        <EmptyState icon={Table2} title="Nenhuma tabela ainda">
          Crie com <code class="rounded bg-muted px-1 py-0.5 text-xs text-foreground">nelcota migrate</code> ou no
          <a href={href('/sql')} class="font-medium text-brand hover:underline">Editor SQL</a>.
        </EmptyState>
      {:else}
        <div class="overflow-hidden rounded-xl border bg-card shadow-card">
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
                      class="group/link inline-flex items-center gap-2.5 font-semibold transition-colors hover:text-brand"
                    >
                      <span
                        class="grid size-7 place-items-center rounded-md border bg-muted/60 text-muted-foreground transition-colors group-hover/link:border-brand/30 group-hover/link:text-brand"
                      >
                        <Table2 class="size-3.5" strokeWidth={1.75} />
                      </span>
                      {table.name}
                    </a>
                    {#if table.kind !== 'table'}
                      <span
                        class="ml-2 rounded-full border px-2 py-0.5 text-2xs font-medium text-muted-foreground">view</span
                      >
                    {/if}
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
