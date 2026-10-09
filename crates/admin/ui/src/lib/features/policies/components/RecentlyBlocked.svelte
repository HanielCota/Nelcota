<script lang="ts">
  import { onMount } from 'svelte'
  import { Button } from '$lib/components/ui/button'
  import { Skeleton } from '$lib/components/ui/skeleton'
  import RefreshCw from '@lucide/svelte/icons/refresh-cw'
  import LoadError from '$lib/components/shared/LoadError.svelte'
  import { RemoteResource } from '$lib/remote-resource.svelte'
  import { api } from '$lib/api'
  import { technical } from '$lib/shared/schema/technical.svelte'
  import { blockReason, blockTarget } from '$lib/features/policies/blocked'
  import type { DeniedRequest, DeniedRequests } from '$lib/types'
  import { errorMessage, intlLocale, t } from '$lib/i18n/index.svelte'

  const PREVIEW = 5
  const resource = new RemoteResource<DeniedRequests>()
  const data = $derived(resource.data)
  let expanded = $state(false)
  const shown = $derived(data ? (expanded ? data.requests : data.requests.slice(0, PREVIEW)) : [])

  async function load() {
    await resource.load((signal) => api.get<DeniedRequests>('/denied', { signal }))
  }
  onMount(() => {
    void load()
    return () => resource.cancel()
  })

  const time = (at: string) =>
    new Date(at).toLocaleString(intlLocale(), { day: '2-digit', month: 'short', hour: '2-digit', minute: '2-digit', second: '2-digit' })

  function who(request: DeniedRequest) {
    if (request.role === 'authenticated' && request.email) return request.email
    const role = request.role as 'anon' | 'invalid_token' | 'service_role' | 'authenticated'
    return ['anon', 'invalid_token', 'service_role', 'authenticated'].includes(role) ? t(`policies.blocked.who.${role}`) : request.role
  }

  function target(request: DeniedRequest) {
    const found = blockTarget(request.path)
    return found ? t(`policies.blocked.target.${found.kind}`, { name: found.name }) : request.path
  }
</script>

<section class="grid gap-4 rounded-3xl bg-card p-5" aria-labelledby="recently-blocked">
  <div class="flex items-start justify-between gap-3">
    <div class="grid gap-0.5">
      <h2 id="recently-blocked" class="text-base font-semibold">{t('policies.blocked.title')}</h2>
      <p class="text-xs text-muted-foreground">{t('policies.blocked.hint', { capacity: data?.capacity ?? 100 })}</p>
      <p class="text-xs text-muted-foreground">{t('policies.blocked.hintMore')}</p>
    </div>
    <Button variant="ghost" size="icon-sm" disabled={resource.loading} onclick={load} aria-label={t('policies.blocked.refresh')} title={t('policies.blocked.refresh')}>
      <RefreshCw class={resource.loading ? 'animate-spin' : ''} />
    </Button>
  </div>

  {#if resource.error}
    <LoadError message={errorMessage(resource.error)} onretry={load} busy={resource.loading} />
  {:else if !data}
    <Skeleton class="h-12" />
  {:else if data.requests.length === 0}
    <p class="rounded-2xl bg-well px-4 py-3.5 text-sm text-muted-foreground">{t('policies.blocked.none')}</p>
  {:else}
    <ul class="divide-y overflow-hidden rounded-2xl bg-well">
      {#each shown as request, index (`${request.at}-${index}`)}
        <li class="grid gap-1 px-4 py-3">
          <div class="flex flex-wrap items-baseline gap-x-2 gap-y-0.5 text-sm">
            <span class="font-medium">{who(request)}</span>
            <span class="text-muted-foreground">·</span>
            <span>{target(request)}</span>
            <span class="w-full text-xs text-muted-foreground tabular-nums">{time(request.at)}</span>
          </div>
          <p class="text-xs text-muted-foreground">{t(`policies.blocked.reasons.${blockReason(request)}`)}</p>
          {#if technical.on || blockReason(request) === 'other'}
            <p class="font-mono text-xs break-all text-muted-foreground">
              {request.method} {request.path} · {request.status} {request.code}{#if request.message} · {request.message}{/if}
            </p>
          {/if}
        </li>
      {/each}
    </ul>
    {#if data.requests.length > PREVIEW}
      <Button variant="ghost" size="sm" class="justify-self-start" onclick={() => (expanded = !expanded)}>
        {expanded ? t('policies.blocked.showLess') : t('policies.blocked.showAll', { count: data.requests.length })}
      </Button>
    {/if}
  {/if}
</section>
