<script lang="ts">
  import { onMount, untrack } from 'svelte'
  import * as Select from '$lib/components/ui/select'
  import BookOpen from '@lucide/svelte/icons/book-open'
  import ArrowUpRight from '@lucide/svelte/icons/arrow-up-right'
  import Globe from '@lucide/svelte/icons/globe'
  import UserRound from '@lucide/svelte/icons/user-round'
  import Server from '@lucide/svelte/icons/server'
  import KeyRound from '@lucide/svelte/icons/key-round'
  import Table2 from '@lucide/svelte/icons/table-2'
  import Plus from '@lucide/svelte/icons/plus'
  import { Skeleton } from '$lib/components/ui/skeleton'
  import { Button } from '$lib/components/ui/button'
  import PageHeader from '$lib/components/app/PageHeader.svelte'
  import CodeBlock from '$lib/components/app/CodeBlock.svelte'
  import ServiceTokenCard from '$lib/components/app/ServiceTokenCard.svelte'
  import LoadError from '$lib/components/app/LoadError.svelte'
  import EmptyState from '$lib/components/app/EmptyState.svelte'
  import { href, navigate, route } from '$lib/router.svelte'
  import { RemoteResource } from '$lib/remote-resource.svelte'
  import { api, enc } from '$lib/api'
  import { authSnippets, tableSnippets, type Lang, type Snippet } from '$lib/snippets'
  import type { TableData, TablesResponse } from '$lib/types'
  import { errorMessage, t } from '$lib/i18n/index.svelte'

  // The panel is served from the same host as the API.
  const base = location.origin

  const tablesResource = new RemoteResource<TablesResponse>()
  const snippetsResource = new RemoteResource<Snippet[]>()
  const tables = $derived(tablesResource.data?.tables ?? [])
  const table = $derived(tables.find((item) => item.name === route.query.get('table'))?.name ?? tables.find((item) => item.kind === 'table')?.name ?? tables[0]?.name ?? '')
  const lang = $derived<Lang>(route.query.get('lang') === 'js' ? 'js' : 'curl')
  const topic = $derived(route.query.get('topic') === 'auth' ? 'auth' : 'tables')
  const snippets = $derived(topic === 'auth' ? authSnippets(base) : snippetsResource.data ?? [])
  const tableError = $derived(tablesResource.error ? errorMessage(tablesResource.error) : '')
  const snippetsError = $derived(snippetsResource.error ? errorMessage(snippetsResource.error) : '')
  const tablesLoading = $derived(tablesResource.loading)
  const snippetsLoading = $derived(snippetsResource.loading)
  async function loadTables() {
    await tablesResource.load(signal => api.get<TablesResponse>('/tables', { signal }))
  }
  onMount(() => { void loadTables(); return () => tablesResource.cancel() })

  function choose(patch: { table?: string; lang?: Lang; topic?: 'tables' | 'auth' }) {
    const query = new URLSearchParams(route.query)
    for (const [key, value] of Object.entries(patch)) query.set(key, value)
    navigate(`/connect?${query}`)
  }

  // Examples for the chosen table, with its real columns (type, DEFAULT, generated).
  async function loadSnippets() {
    snippetsResource.clear()
    if (topic === 'auth' || !table) return
    const selectedTable = table
    await snippetsResource.load(async signal => {
      const data = await api.get<TableData>(`/tables/${enc(selectedTable)}?size=1`, { signal })
      return tableSnippets(base, selectedTable, data.table.columns.map(c => ({
        name: c.name, type: c.full_type, has_default: c.has_default, generated: c.generated, nullable: c.nullable,
      })))
    })
  }

  $effect(() => {
    void [topic, table]
    untrack(loadSnippets)
    return () => snippetsResource.cancel()
  })

  const endpoints = [
    { id: 'rest', label: 'REST', path: '/rest/v1/' },
    { id: 'auth', label: 'Auth', path: '/auth/v1/' },
    { id: 'jwks', label: 'JWKS', path: '/auth/v1/.well-known/jwks.json' },
  ] as const

  const roles = ['anon', 'authenticated', 'service_role'] as const
  const roleIcons = { anon: Globe, authenticated: UserRound, service_role: Server }

  const tab = (active: boolean) =>
    [
      'h-7 cursor-pointer rounded px-3 text-sm transition-colors',
      active ? 'border border-border bg-card text-foreground' : 'text-muted-foreground hover:text-foreground',
    ]
</script>

<div class="mx-auto grid max-w-5xl gap-10 px-4 py-8 *:min-w-0 sm:px-6 lg:px-10 lg:py-10">
  <PageHeader title={t('connect.title')} description={t('connect.description')}>
    {#snippet actions()}
      <Button variant="outline" href="/rest/v1/" target="_blank" rel="noopener">
        <BookOpen />{t('connect.openapi')}<ArrowUpRight class="text-muted-foreground" />
      </Button>
    {/snippet}
  </PageHeader>

  <nav class="-mt-6 flex flex-wrap gap-x-5 gap-y-2 text-sm" aria-label={t('connect.navigation')}><a class="text-muted-foreground underline-offset-4 hover:text-foreground hover:underline" href="#api-address">{t('connect.address')}</a><a class="text-muted-foreground underline-offset-4 hover:text-foreground hover:underline" href="#api-roles">{t('connect.caller')}</a><a class="text-muted-foreground underline-offset-4 hover:text-foreground hover:underline" href="#api-examples">{t('connect.examples')}</a><a class="text-muted-foreground underline-offset-4 hover:text-foreground hover:underline" href="#api-token">{t('connect.token.title')}</a></nav>

  <section id="api-address" class="-mt-2 grid scroll-mt-6 gap-4">
    <h2 class="text-base font-semibold">{t('connect.address')}</h2>
    <CodeBlock code={base} label={t('connect.copyAddress')} />
    <dl class="grid gap-2 text-sm sm:grid-cols-[6rem_auto_1fr] sm:gap-x-4">
      {#each endpoints as endpoint (endpoint.path)}
        <dt class="font-medium">{endpoint.label}</dt>
        <dd><code class="text-xs">{endpoint.path}</code></dd>
        <dd class="text-muted-foreground max-sm:mb-2">{t(`connect.endpoints.${endpoint.id}`)}</dd>
      {/each}
    </dl>
  </section>

  <section id="api-roles" class="grid scroll-mt-6 gap-4">
    <div>
      <h2 class="text-base font-semibold">{t('connect.caller')}</h2>
      <p class="mt-1 text-sm text-muted-foreground">{t('connect.callerHint')}</p>
    </div>
    <div class="grid gap-3 md:grid-cols-3">
      {#each roles as role (role)}
        {@const Icon = roleIcons[role]}
        <div class="rounded-lg border bg-card p-4">
          <p class="flex items-center gap-2 text-sm font-medium"><Icon class="size-4 text-muted-foreground" aria-hidden="true" />{t(`connect.roles.${role}.title`)}</p>
          <code class="text-xs text-muted-foreground">{role}</code>
          <p class="mt-2 text-sm text-muted-foreground">{t(`connect.roles.${role}.text`)}</p>
          {#if role === 'service_role'}
            <p class="mt-2 text-sm text-warning">{t('connect.neverInBrowser')}</p>
          {/if}
        </div>
      {/each}
    </div>
  </section>

  <section id="api-examples" class="grid scroll-mt-6 gap-4">
    <h2 class="text-base font-semibold">{t('connect.examples')}</h2>
    <div class="flex flex-wrap items-center gap-3">
      <nav class="flex h-9 items-center gap-1 rounded-md bg-muted p-1" aria-label={t('connect.topic')}>
        <button type="button" class={tab(topic === 'tables')} aria-pressed={topic === 'tables'} onclick={() => choose({ topic: 'tables' })}
          >{t('connect.topics.tables')}</button
        >
        <button type="button" class={tab(topic === 'auth')} aria-pressed={topic === 'auth'} onclick={() => choose({ topic: 'auth' })}
          >{t('connect.topics.auth')}</button
        >
      </nav>
      {#if topic === 'tables' && tables.length}
        <Select.Root type="single" value={table} onValueChange={(table) => choose({ table })}>
          <Select.Trigger class="w-52 font-mono text-xs" aria-label={t('connect.table')}>{table}</Select.Trigger>
          <Select.Content>
            {#each tables as t (t.name)}
              <Select.Item value={t.name} class="font-mono text-xs">{t.name}</Select.Item>
            {/each}
          </Select.Content>
        </Select.Root>
      {/if}
      <nav class="flex h-9 items-center gap-1 rounded-md bg-muted p-1 sm:ml-auto" aria-label={t('connect.language')}>
        <button type="button" class={tab(lang === 'curl')} aria-pressed={lang === 'curl'} onclick={() => choose({ lang: 'curl' })}
          >curl</button
        >
        <button type="button" class={tab(lang === 'js')} aria-pressed={lang === 'js'} onclick={() => choose({ lang: 'js' })}
          >JavaScript</button
        >
      </nav>
    </div>

    {#if topic === 'tables' && tableError}<LoadError message={tableError} onretry={loadTables} busy={tablesLoading} />{/if}
    {#if snippetsError}<LoadError message={snippetsError} onretry={loadSnippets} busy={snippetsLoading} />{/if}
    {#if topic === 'tables' && (tablesLoading || snippetsLoading)}<Skeleton class="h-48" />
    {:else if topic === 'tables' && !tables.length && !tableError}
      <EmptyState icon={Table2} title={t('connect.noTables')}>{#snippet actions()}<Button href={href('/tables?create=true')}><Plus data-icon="inline-start" aria-hidden="true" />{t('tables.editor.newTable')}</Button>{/snippet}</EmptyState>
    {:else}
      <div class="grid gap-6 *:min-w-0">
        {#each snippets as snippet (snippet.id)}
          <article class="grid gap-2 *:min-w-0">
            <div>
              <h3 class="text-sm font-medium">{t(`connect.snippets.${snippet.id}.label`)}</h3>
              <p class="text-sm text-muted-foreground">{t(`connect.snippets.${snippet.id}.description`, snippet.params)}</p>
            </div>
            <CodeBlock code={snippet.code[lang]} />
          </article>
        {/each}
      </div>
    {/if}
    <p class="text-sm text-muted-foreground">
      {t('connect.types')} <code class="text-xs text-foreground">nelcota types -o database.ts</code>
    </p>
  </section>
  <details id="api-token" class="scroll-mt-6 rounded-lg border bg-card p-4"><summary class="flex cursor-pointer items-center gap-2 text-sm font-medium"><KeyRound class="size-4 text-muted-foreground" aria-hidden="true" />{t('connect.token.title')} <code>service_role</code></summary><div class="mt-4"><ServiceTokenCard /></div></details>
</div>
