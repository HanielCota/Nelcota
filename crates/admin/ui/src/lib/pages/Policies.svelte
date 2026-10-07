<script lang="ts">
  import { onMount } from 'svelte'
  import { Skeleton } from '$lib/components/ui/skeleton'
  import { Button } from '$lib/components/ui/button'
  import Table2 from '@lucide/svelte/icons/table-2'
  import ShieldCheck from '@lucide/svelte/icons/shield-check'
  import SquareFunction from '@lucide/svelte/icons/square-function'
  import Plus from '@lucide/svelte/icons/plus'
  import Pencil from '@lucide/svelte/icons/pencil'
  import Trash2 from '@lucide/svelte/icons/trash-2'
  import { toast } from 'svelte-sonner'
  import PageHeader from '$lib/components/app/PageHeader.svelte'
  import RlsBadge from '$lib/components/app/RlsBadge.svelte'
  import PolicySheet from '$lib/components/app/PolicySheet.svelte'
  import ConfirmDialog from '$lib/components/app/ConfirmDialog.svelte'
  import Callout from '$lib/components/app/Callout.svelte'
  import EmptyState from '$lib/components/app/EmptyState.svelte'
  import { api } from '$lib/api'
  import { ddl, toPolicyDef, type PolicyDef } from '$lib/ddl'
  import { href } from '$lib/router.svelte'
  import type { PoliciesData } from '$lib/types'

  let data = $state<PoliciesData | null>(null)
  let error = $state('')

  async function load() {
    try {
      data = await api.get<PoliciesData>('/policies')
      error = ''
    } catch (e) {
      error = (e as Error).message
    }
  }

  onMount(load)

  // Formulário: tabela alvo e policy em edição (`null` = nova).
  let sheetOpen = $state(false)
  let sheetTable = $state('')
  let editing = $state<PolicyDef | null>(null)
  let toDelete = $state<{ table: string; policy: string } | null>(null)
  let deleteOpen = $state(false)

  function openSheet(table: string, policy: PolicyDef | null) {
    sheetTable = table
    editing = policy
    sheetOpen = true
  }

  async function enableRls(table: string) {
    try {
      const result = await ddl.alterTable(table, [{ action: 'set_rls', enabled: true }])
      toast.success(result.message ?? 'RLS ativado')
      await load()
    } catch (e) {
      toast.error((e as Error).message)
    }
  }

  async function dropPolicy() {
    if (!toDelete) return
    try {
      const result = await ddl.dropPolicy(toDelete.table, toDelete.policy)
      toast.success(result.message ?? 'Policy apagada')
      await load()
    } catch (e) {
      toast.error((e as Error).message)
      throw e
    }
  }

  // Cada comando com uma tinta própria, para bater o olho na lista.
  const commandTint: Record<string, string> = {
    select: 'border-sky-500/30 bg-sky-500/10 text-sky-700 dark:text-sky-300',
    insert: 'border-brand/30 bg-brand/10 text-brand',
    update: 'border-warning/30 bg-warning/10 text-warning',
    delete: 'border-destructive/30 bg-destructive/10 text-destructive',
    all: 'border-violet-500/30 bg-violet-500/10 text-violet-700 dark:text-violet-300',
  }
</script>

<div class="mx-auto max-w-6xl px-4 py-8 sm:px-6 lg:px-10 lg:py-12">
  <PageHeader
    title="Policies"
    icon={ShieldCheck}
    description="Row Level Security por tabela: quem lê e altera cada linha."
  />

  {#if error}
    <Callout variant="danger" title="Não foi possível carregar as policies">{error}</Callout>
  {:else if !data}
    <div class="grid gap-4">
      {#each [0, 1, 2] as i (i)}<Skeleton class="h-40 rounded-xl" />{/each}
    </div>
  {:else}
    {#if data.exposed_without_rls.length}
      <Callout variant="danger" title={`Expostas sem RLS: ${data.exposed_without_rls.join(', ')}`} class="mb-6">
        Quem tem GRANT nessas tabelas lê e altera todas as linhas. Ative o RLS e crie policies.
      </Callout>
    {/if}

    {#if data.tables.length === 0}
      <EmptyState
        icon={Table2}
        title="Nenhuma tabela ainda"
        description="Crie uma tabela para definir quem lê e altera as linhas dela."
      />
    {/if}
    <div class="grid gap-5">
      {#each data.tables as table (table.name)}
        <section class="overflow-hidden rounded-xl border bg-card shadow-card">
          <header class="flex flex-wrap items-center gap-3 border-b bg-muted/40 px-5 py-3.5">
            <span class="grid size-8 place-items-center rounded-lg border bg-card text-muted-foreground">
              <Table2 class="size-4" strokeWidth={1.75} />
            </span>
            <h2 class="text-base font-semibold">
              <a href={href(`/tables/${encodeURIComponent(table.name)}`)} class="transition-colors hover:text-brand"
                >{table.name}</a
              >
            </h2>
            <RlsBadge rls={table.rls} />
            <div class="ml-auto flex items-center gap-2">
              {#if !table.rls.enabled}
                <Button variant="outline" size="sm" onclick={() => enableRls(table.name)}><ShieldCheck />Ativar RLS</Button>
              {/if}
              <Button size="sm" onclick={() => openSheet(table.name, null)}><Plus />Nova policy</Button>
            </div>
          </header>
          {#if table.policies.length === 0}
            <p class="px-5 py-6 text-sm text-muted-foreground">
              {#if table.rls.enabled}
                Nenhuma policy: só <code class="text-xs text-foreground">service_role</code> e o dono acessam.
              {:else}
                Nenhuma policy.
              {/if}
            </p>
          {:else}
            <div class="divide-y">
              {#each table.policies as policy (policy.name)}
                <div
                  class="group grid gap-3 px-5 py-4 text-sm transition-colors hover:bg-muted/30 md:grid-cols-[16rem_12rem_1fr_auto] md:items-start"
                >
                  <p class="font-semibold">
                    {policy.name}
                    {#if !policy.permissive}<span
                        class="ml-1.5 rounded-full border border-border-strong bg-muted px-2 py-px text-2xs font-medium text-muted-foreground"
                        >restritiva</span
                      >{/if}
                  </p>
                  <div class="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
                    <span
                      class={[
                        'rounded-md border px-2 py-0.5 font-mono text-2xs font-semibold uppercase',
                        commandTint[policy.command.toLowerCase()] ?? 'border-border-strong bg-muted text-foreground',
                      ]}>{policy.command}</span
                    >
                    {policy.roles.join(', ')}
                  </div>
                  <div class="grid min-w-0 gap-1.5 font-mono text-xs">
                    {#if policy.using}
                      <p class="rounded-md bg-muted/60 px-2.5 py-1.5 break-words">
                        <span class="font-semibold text-muted-foreground">using</span> {policy.using}
                      </p>
                    {/if}
                    {#if policy.check}
                      <p class="rounded-md bg-muted/60 px-2.5 py-1.5 break-words">
                        <span class="font-semibold text-muted-foreground">with check</span> {policy.check}
                      </p>
                    {/if}
                  </div>
                  <div class="flex items-start justify-end gap-1 transition-opacity md:opacity-50 md:group-hover:opacity-100 md:focus-within:opacity-100">
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      aria-label={`Editar ${policy.name}`}
                      onclick={() => openSheet(table.name, toPolicyDef(policy))}><Pencil /></Button
                    >
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      aria-label={`Apagar ${policy.name}`}
                      onclick={() => {
                        toDelete = { table: table.name, policy: policy.name }
                        deleteOpen = true
                      }}><Trash2 /></Button
                    >
                  </div>
                </div>
              {/each}
            </div>
          {/if}
        </section>
      {/each}
    </div>

    {#if data.anon_functions.length}
      <section class="mt-10 rounded-xl border bg-card p-5 shadow-card sm:p-6">
        <h2 class="flex items-center gap-2 text-base font-semibold">
          <SquareFunction class="size-[18px] text-muted-foreground" strokeWidth={1.75} />Funções que anon pode executar
        </h2>
        <div class="mt-3 flex flex-wrap gap-1.5">
          {#each data.anon_functions as fn (fn)}
            <code class="rounded-md border border-border-strong bg-muted px-2 py-0.5 text-xs">{fn}</code>
          {/each}
        </div>
        <p class="mt-4 text-sm text-muted-foreground">
          O Postgres dá EXECUTE a PUBLIC por padrão. Para restringir:
          <code class="text-xs text-foreground">revoke execute on function f() from public</code>.
        </p>
      </section>
    {/if}
  {/if}
</div>

<PolicySheet bind:open={sheetOpen} table={sheetTable} original={editing} onsaved={load} />
{#if toDelete}
  <ConfirmDialog
    bind:open={deleteOpen}
    title={`Apagar a policy "${toDelete.policy}"?`}
    description={`As linhas de ${toDelete.table} que só ela liberava deixam de ser acessíveis.`}
    confirmLabel="Apagar policy"
    destructive
    onconfirm={dropPolicy}
  />
{/if}
