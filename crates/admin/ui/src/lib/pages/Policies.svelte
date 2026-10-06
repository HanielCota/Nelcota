<script lang="ts">
  import { onMount } from 'svelte'
  import * as Card from '$lib/components/ui/card'
  import * as Alert from '$lib/components/ui/alert'
  import { Button } from '$lib/components/ui/button'
  import { Skeleton } from '$lib/components/ui/skeleton'
  import Lock from '@lucide/svelte/icons/lock'
  import LockOpen from '@lucide/svelte/icons/lock-open'
  import TriangleAlert from '@lucide/svelte/icons/triangle-alert'
  import CircleCheck from '@lucide/svelte/icons/circle-check'
  import SquareFunction from '@lucide/svelte/icons/square-function'
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

<div class="mx-auto max-w-6xl p-6 lg:p-8">
  <PageHeader
    title="Policies"
    description="Row Level Security: quem enxerga e altera cada linha. A API nunca decide permissão; o Postgres decide."
  />

  {#if error}
    <Alert.Root variant="destructive"><Alert.Title>{error}</Alert.Title></Alert.Root>
  {:else if !data}
    <div class="space-y-4">{#each Array(3) as _, i (i)}<Skeleton class="h-28 rounded-xl" />{/each}</div>
  {:else}
    {#if data.exposed_without_rls.length === 0}
      <Alert.Root class="mb-6 border-primary/30 bg-primary/5">
        <CircleCheck class="text-primary" />
        <Alert.Title>Nenhuma tabela exposta sem RLS.</Alert.Title>
      </Alert.Root>
    {:else}
      {#each data.exposed_without_rls as name (name)}
        <Alert.Root variant="destructive" class="mb-3 border-destructive/40 bg-destructive/5">
          <TriangleAlert />
          <Alert.Title><code class="font-mono">{name}</code> está exposta sem RLS</Alert.Title>
          <Alert.Description>Quem tem GRANT vê e altera todas as linhas.</Alert.Description>
        </Alert.Root>
      {/each}
      <div class="mb-6"></div>
    {/if}

    <div class="space-y-4">
      {#each data.tables as table (table.name)}
        <Card.Root class="gap-0 py-0">
          <div class="flex flex-wrap items-center gap-2.5 border-b px-4 py-3">
            {#if table.rls.enabled}<Lock class="size-4 text-muted-foreground" />{:else}<LockOpen class="size-4 text-muted-foreground" />{/if}
            <h2 class="font-semibold">{table.name}</h2>
            <RlsBadge rls={table.rls} />
            <Button variant="ghost" size="sm" class="ml-auto" href={href(`/tables/${encodeURIComponent(table.name)}`)}>Ver dados</Button>
          </div>
          {#if table.policies.length === 0}
            <p class="px-4 py-6 text-center text-sm text-muted-foreground">
              {#if table.rls.enabled}
                RLS ligado e nenhuma policy: só <code class="font-mono">service_role</code> (e o dono) acessam.
              {:else}
                Sem policies.
              {/if}
            </p>
          {:else}
            <div class="divide-y">
              {#each table.policies as policy (policy.name)}
                <div class="grid gap-3 px-4 py-3 md:grid-cols-[minmax(180px,1fr)_auto_2fr] md:items-start">
                  <div class="flex items-center gap-2 font-medium">
                    {policy.name}
                    {#if !policy.permissive}
                      <span class="rounded-full border px-1.5 text-[10px] text-muted-foreground">restritiva</span>
                    {/if}
                  </div>
                  <div class="flex items-center gap-2 text-xs">
                    <span class="rounded border bg-muted px-1.5 py-px font-mono text-[10.5px] font-semibold">{policy.command}</span>
                    <span class="text-muted-foreground">{policy.roles.join(', ')}</span>
                  </div>
                  <div class="grid gap-1.5 text-xs">
                    {#if policy.using}
                      <div class="flex items-start gap-2">
                        <span class="w-20 shrink-0 pt-0.5 text-muted-foreground">USING</span>
                        <code class="rounded bg-muted px-1.5 py-0.5 font-mono break-all">{policy.using}</code>
                      </div>
                    {/if}
                    {#if policy.check}
                      <div class="flex items-start gap-2">
                        <span class="w-20 shrink-0 pt-0.5 text-muted-foreground">WITH CHECK</span>
                        <code class="rounded bg-muted px-1.5 py-0.5 font-mono break-all">{policy.check}</code>
                      </div>
                    {/if}
                  </div>
                </div>
              {/each}
            </div>
          {/if}
        </Card.Root>
      {/each}
    </div>

    {#if data.anon_functions.length}
      <Card.Root class="mt-6 gap-3">
        <Card.Header>
          <Card.Title class="flex items-center gap-2 text-sm"><SquareFunction class="size-4" />Funções executáveis por anon</Card.Title>
          <Card.Description>
            O Postgres concede EXECUTE a PUBLIC por padrão. Para restringir:
            <code class="font-mono">REVOKE EXECUTE ON FUNCTION f() FROM PUBLIC</code>.
          </Card.Description>
        </Card.Header>
        <Card.Content class="flex flex-wrap gap-1.5">
          {#each data.anon_functions as fn (fn)}
            <code class="rounded border bg-muted px-1.5 py-0.5 font-mono text-xs">{fn}</code>
          {/each}
        </Card.Content>
      </Card.Root>
    {/if}
  {/if}
</div>
