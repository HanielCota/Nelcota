<script lang="ts">
  import { onMount } from 'svelte'
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

<div class="mx-auto max-w-5xl p-6">
  <PageHeader title="Policies" />

  {#if error}
    <p class="text-sm text-destructive">{error}</p>
  {:else if !data}
    <p class="text-sm text-muted-foreground">Carregando…</p>
  {:else}
    {#if data.exposed_without_rls.length}
      <div class="mb-5 rounded border border-destructive/40 px-4 py-3 text-sm">
        <p class="font-medium text-destructive">Expostas sem RLS: {data.exposed_without_rls.join(', ')}</p>
        <p class="mt-1 text-muted-foreground">Quem tem GRANT nessas tabelas lê e altera todas as linhas.</p>
      </div>
    {/if}

    <div class="space-y-6">
      {#each data.tables as table (table.name)}
        <section>
          <div class="mb-2 flex items-baseline gap-3">
            <h2 class="font-medium">
              <a href={href(`/tables/${encodeURIComponent(table.name)}`)} class="hover:underline">{table.name}</a>
            </h2>
            <RlsBadge rls={table.rls} />
          </div>
          {#if table.policies.length === 0}
            <p class="text-sm text-muted-foreground">
              {#if table.rls.enabled}
                Nenhuma policy: só <span class="font-mono">service_role</span> e o dono acessam.
              {:else}
                Nenhuma policy.
              {/if}
            </p>
          {:else}
            <div class="divide-y rounded border">
              {#each table.policies as policy (policy.name)}
                <div class="grid gap-2 px-4 py-3 text-sm md:grid-cols-[14rem_10rem_1fr]">
                  <p>
                    {policy.name}
                    {#if !policy.permissive}<span class="text-xs text-muted-foreground">(restritiva)</span>{/if}
                  </p>
                  <p class="text-xs text-muted-foreground">
                    <span class="font-mono text-foreground">{policy.command.toLowerCase()}</span>
                    para {policy.roles.join(', ')}
                  </p>
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
      <section class="mt-8">
        <h2 class="mb-2 font-medium">Funções que anon pode executar</h2>
        <p class="font-mono text-xs">{data.anon_functions.join(', ')}</p>
        <p class="mt-2 text-sm text-muted-foreground">
          O Postgres dá EXECUTE a PUBLIC por padrão. Para restringir:
          <span class="font-mono">revoke execute on function f() from public</span>.
        </p>
      </section>
    {/if}
  {/if}
</div>
