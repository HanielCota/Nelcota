<script lang="ts">
  import { onMount } from 'svelte'
  import { Skeleton } from '$lib/components/ui/skeleton'
  import { Button } from '$lib/components/ui/button'
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

</script>

<div class="mx-auto max-w-6xl px-4 py-8 sm:px-6 lg:px-10 lg:py-10">
  <PageHeader title="Policies" description="Row Level Security por tabela: quem lê e altera cada linha." />

  {#if error}
    <p class="text-sm text-destructive">{error}</p>
  {:else if !data}
    <div class="grid gap-4">
      {#each [0, 1, 2] as i (i)}<Skeleton class="h-36 rounded-lg" />{/each}
    </div>
  {:else}
    {#if data.exposed_without_rls.length}
      <Callout variant="danger" title={`Expostas sem RLS: ${data.exposed_without_rls.join(', ')}`} class="mb-6">
        Quem tem GRANT nessas tabelas lê e altera todas as linhas.
      </Callout>
    {/if}

    {#if data.tables.length === 0}
      <EmptyState title="Nenhuma tabela" description="Crie uma tabela para definir policies nela." />
    {/if}
    <div class="grid gap-4">
      {#each data.tables as table (table.name)}
        <section class="overflow-hidden rounded-lg border bg-card">
          <header class="flex flex-wrap items-center gap-3 border-b bg-muted/40 px-4 py-2.5">
            <h2 class="text-sm font-semibold">
              <a href={href(`/tables/${encodeURIComponent(table.name)}`)} class="hover:underline">{table.name}</a>
            </h2>
            <RlsBadge rls={table.rls} />
            <div class="ml-auto flex items-center gap-2">
              {#if !table.rls.enabled}
                <Button variant="outline" size="sm" onclick={() => enableRls(table.name)}>Ativar RLS</Button>
              {/if}
              <Button variant="outline" size="sm" onclick={() => openSheet(table.name, null)}><Plus />Nova policy</Button>
            </div>
          </header>
          {#if table.policies.length === 0}
            <p class="px-4 py-5 text-sm text-muted-foreground">
              {#if table.rls.enabled}
                Nenhuma policy: só <code class="text-xs text-foreground">service_role</code> e o dono acessam.
              {:else}
                Nenhuma policy.
              {/if}
            </p>
          {:else}
            <div class="divide-y">
              {#each table.policies as policy (policy.name)}
                <div class="grid gap-3 px-4 py-3.5 text-sm md:grid-cols-[16rem_12rem_1fr_auto] md:items-start">
                  <p class="font-medium">
                    {policy.name}
                    {#if !policy.permissive}<span class="ml-1 text-xs font-normal text-muted-foreground">(restritiva)</span>{/if}
                  </p>
                  <p class="text-xs text-muted-foreground">
                    <span class="mr-1.5 rounded border px-1.5 py-px font-mono text-foreground">{policy.command}</span>
                    {policy.roles.join(', ')}
                  </p>
                  <div class="grid min-w-0 gap-1 font-mono text-xs">
                    {#if policy.using}
                      <p class="break-words"><span class="text-muted-foreground">using</span> {policy.using}</p>
                    {/if}
                    {#if policy.check}
                      <p class="break-words"><span class="text-muted-foreground">with check</span> {policy.check}</p>
                    {/if}
                  </div>
                  <div class="flex items-start justify-end gap-1">
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      aria-label={`Editar ${policy.name}`}
                      title="Editar"
                      onclick={() => openSheet(table.name, toPolicyDef(policy))}><Pencil /></Button
                    >
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      aria-label={`Apagar ${policy.name}`}
                      title="Apagar"
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
      <section class="mt-8 rounded-lg border bg-card p-5">
        <h2 class="text-sm font-semibold">Funções que anon pode executar</h2>
        <p class="mt-2 font-mono text-xs">{data.anon_functions.join(', ')}</p>
        <p class="mt-3 text-sm text-muted-foreground">
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
