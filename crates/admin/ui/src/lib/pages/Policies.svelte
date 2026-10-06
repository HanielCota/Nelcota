<script lang="ts">
  import { onMount } from 'svelte'
  import { Skeleton } from '$lib/components/ui/skeleton'
  import Table2 from '@lucide/svelte/icons/table-2'
  import TriangleAlert from '@lucide/svelte/icons/triangle-alert'
  import PageHeader from '$lib/components/app/PageHeader.svelte'
  import RlsBadge from '$lib/components/app/RlsBadge.svelte'
  import { api } from '$lib/api'
  import { href } from '$lib/router.svelte'
  import type { PoliciesData } from '$lib/types'

  let data = $state<PoliciesData | null>(null)
  let error = $state('')

  onMount(async () => {
    try {
      data = await api.get<PoliciesData>('/policies')
    } catch (e) {
      error = (e as Error).message
    }
  })
</script>

<div class="mx-auto max-w-6xl px-6 py-10 lg:px-10">
  <PageHeader title="Policies" description="Row Level Security por tabela: quem lê e altera cada linha." />

  {#if error}
    <p class="rounded-md border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{error}</p>
  {:else if !data}
    <div class="grid gap-4">
      {#each [0, 1] as i (i)}<Skeleton class="h-36 rounded-lg" />{/each}
    </div>
  {:else}
    {#if data.exposed_without_rls.length}
      <div class="mb-6 flex gap-3 rounded-lg border border-destructive/30 bg-destructive/5 p-4 text-sm">
        <TriangleAlert class="mt-0.5 size-4 shrink-0 text-destructive" />
        <div>
          <p class="font-medium text-destructive">Expostas sem RLS: {data.exposed_without_rls.join(', ')}</p>
          <p class="mt-1 font-light text-muted-foreground">Quem tem GRANT nessas tabelas lê e altera todas as linhas.</p>
        </div>
      </div>
    {/if}

    <div class="grid gap-4">
      {#each data.tables as table (table.name)}
        <section class="overflow-hidden rounded-lg border bg-card">
          <header class="flex flex-wrap items-center gap-3 border-b bg-muted/40 px-4 py-3">
            <Table2 class="size-4 text-muted-foreground" strokeWidth={1.6} />
            <h2 class="text-sm font-medium">
              <a href={href(`/tables/${encodeURIComponent(table.name)}`)} class="hover:text-brand">{table.name}</a>
            </h2>
            <RlsBadge rls={table.rls} />
          </header>
          {#if table.policies.length === 0}
            <p class="px-4 py-5 text-sm font-light text-muted-foreground">
              {#if table.rls.enabled}
                Nenhuma policy: só <code class="text-xs text-foreground">service_role</code> e o dono acessam.
              {:else}
                Nenhuma policy.
              {/if}
            </p>
          {:else}
            <div class="divide-y">
              {#each table.policies as policy (policy.name)}
                <div class="grid gap-3 px-4 py-3.5 text-sm md:grid-cols-[16rem_11rem_1fr]">
                  <p class="font-medium">
                    {policy.name}
                    {#if !policy.permissive}<span class="ml-1 text-xs font-normal text-muted-foreground">(restritiva)</span>{/if}
                  </p>
                  <div class="flex flex-wrap items-center gap-1.5 text-xs text-muted-foreground">
                    <span
                      class="rounded border border-border-strong bg-muted px-1.5 py-px font-mono text-[11px] text-foreground uppercase"
                      >{policy.command}</span
                    >
                    {policy.roles.join(', ')}
                  </div>
                  <div class="grid gap-1 font-mono text-xs">
                    {#if policy.using}<p><span class="text-muted-foreground">using</span> {policy.using}</p>{/if}
                    {#if policy.check}<p><span class="text-muted-foreground">with check</span> {policy.check}</p>{/if}
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
        <h2 class="text-sm font-medium">Funções que anon pode executar</h2>
        <p class="mt-2 font-mono text-xs">{data.anon_functions.join(', ')}</p>
        <p class="mt-3 text-sm font-light text-muted-foreground">
          O Postgres dá EXECUTE a PUBLIC por padrão. Para restringir:
          <code class="text-xs text-foreground">revoke execute on function f() from public</code>.
        </p>
      </section>
    {/if}
  {/if}
</div>
