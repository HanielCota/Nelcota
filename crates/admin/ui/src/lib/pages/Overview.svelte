<script lang="ts">
  import { onMount } from 'svelte'
  import * as Card from '$lib/components/ui/card'
  import * as Table from '$lib/components/ui/table'
  import * as Alert from '$lib/components/ui/alert'
  import { Button } from '$lib/components/ui/button'
  import { Skeleton } from '$lib/components/ui/skeleton'
  import Table2 from '@lucide/svelte/icons/table-2'
  import Users from '@lucide/svelte/icons/users'
  import ShieldCheck from '@lucide/svelte/icons/shield-check'
  import SquareFunction from '@lucide/svelte/icons/square-function'
  import TriangleAlert from '@lucide/svelte/icons/triangle-alert'
  import SquareTerminal from '@lucide/svelte/icons/square-terminal'
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

  const cards = $derived(
    data
      ? [
          { label: 'Tabelas e views', value: data.counts.tables, icon: Table2, path: '/tables' },
          { label: 'Usuários', value: data.counts.users, icon: Users, path: '/users' },
          { label: 'Policies RLS', value: data.counts.policies, icon: ShieldCheck, path: '/policies' },
          { label: 'Funções (RPC)', value: data.counts.functions, icon: SquareFunction, path: '/sql' },
        ]
      : [],
  )

  const fmt = new Intl.NumberFormat('pt-BR')
</script>

<div class="mx-auto max-w-6xl p-6 lg:p-8">
  <PageHeader
    title="Visão geral"
    description={data ? `Schema exposto "${data.schema}": a API REST, o auth e o painel leem daqui.` : undefined}
  />

  {#if error}
    <Alert.Root variant="destructive"><Alert.Title>{error}</Alert.Title></Alert.Root>
  {/if}

  <div class="mb-6 grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
    {#if !data}
      {#each Array(4) as _, i (i)}<Skeleton class="h-[92px] rounded-xl" />{/each}
    {:else}
      {#each cards as card (card.label)}
        <a href={href(card.path)} class="group">
          <Card.Root class="gap-1 py-4 transition-colors group-hover:border-primary/40">
            <Card.Header class="px-4">
              <Card.Description class="flex items-center gap-2 text-xs">
                <card.icon class="size-3.5" />{card.label}
              </Card.Description>
            </Card.Header>
            <Card.Content class="px-4">
              <span class="text-2xl font-semibold tracking-tight">{fmt.format(card.value)}</span>
            </Card.Content>
          </Card.Root>
        </a>
      {/each}
    {/if}
  </div>

  {#if data && data.exposed_without_rls.length}
    <Alert.Root variant="destructive" class="mb-6 border-destructive/40 bg-destructive/5">
      <TriangleAlert />
      <Alert.Title>Tabelas expostas sem RLS</Alert.Title>
      <Alert.Description>
        <p>
          {#each data.exposed_without_rls as name, i (name)}<code class="font-mono font-semibold">{name}</code
            >{i < data.exposed_without_rls.length - 1 ? ', ' : ''}{/each}: qualquer role com GRANT (inclusive
          <code class="font-mono">anon</code>, se concedido) lê e escreve <em>todas</em> as linhas. Ative com
          <code class="font-mono">ALTER TABLE … ENABLE ROW LEVEL SECURITY</code> e crie policies, ou revogue os GRANTs.
        </p>
      </Alert.Description>
    </Alert.Root>
  {/if}

  <Card.Root class="gap-0 py-0">
    <div class="flex items-center gap-2 border-b px-4 py-3">
      <h2 class="text-sm font-semibold">Tabelas</h2>
      <Button variant="outline" size="sm" class="ml-auto" href={href('/sql')}>
        <SquareTerminal />Nova tabela via SQL
      </Button>
    </div>
    {#if !data}
      <div class="space-y-2 p-4">{#each Array(3) as _, i (i)}<Skeleton class="h-8" />{/each}</div>
    {:else if data.tables.length === 0}
      <p class="px-4 py-10 text-center text-sm text-muted-foreground">
        Nenhuma tabela ainda. Crie uma com <code class="font-mono">nelcota migrate</code> ou pelo editor SQL.
      </p>
    {:else}
      <Table.Root>
        <Table.Header>
          <Table.Row class="hover:bg-transparent">
            <Table.Head class="pl-4">Nome</Table.Head>
            <Table.Head>Linhas</Table.Head>
            <Table.Head>RLS</Table.Head>
            <Table.Head>anon</Table.Head>
            <Table.Head>authenticated</Table.Head>
          </Table.Row>
        </Table.Header>
        <Table.Body>
          {#each data.tables as table (table.name)}
            <Table.Row>
              <Table.Cell class="pl-4">
                <a href={href(`/tables/${encodeURIComponent(table.name)}`)} class="flex items-center gap-2 font-medium hover:text-primary">
                  <Table2 class="size-4 text-muted-foreground" />{table.name}
                  {#if table.kind !== 'table'}<span class="text-xs font-normal text-muted-foreground">{table.kind}</span>{/if}
                </a>
              </Table.Cell>
              <Table.Cell class="font-mono text-xs">
                {table.rows === null ? '—' : `${table.rows_exact ? '' : '≈ '}${fmt.format(table.rows)}`}
              </Table.Cell>
              <Table.Cell><RlsBadge rls={table.rls} /></Table.Cell>
              <Table.Cell><Grants grants={table.grants.anon} /></Table.Cell>
              <Table.Cell><Grants grants={table.grants.authenticated} /></Table.Cell>
            </Table.Row>
          {/each}
        </Table.Body>
      </Table.Root>
    {/if}
  </Card.Root>
</div>
