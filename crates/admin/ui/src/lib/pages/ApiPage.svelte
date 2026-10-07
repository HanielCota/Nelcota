<script lang="ts">
  import { onMount } from 'svelte'
  import * as Select from '$lib/components/ui/select'
  import BookOpen from '@lucide/svelte/icons/book-open'
  import ArrowUpRight from '@lucide/svelte/icons/arrow-up-right'
  import { Button } from '$lib/components/ui/button'
  import PageHeader from '$lib/components/app/PageHeader.svelte'
  import CodeBlock from '$lib/components/app/CodeBlock.svelte'
  import ServiceTokenCard from '$lib/components/app/ServiceTokenCard.svelte'
  import { api, enc, isAbort } from '$lib/api'
  import { authSnippets, tableSnippets, type Lang, type Snippet } from '$lib/snippets'
  import type { TableData, TableSummary } from '$lib/types'
  import { t } from '$lib/i18n/index.svelte'

  // The panel is served from the same host as the API.
  const base = location.origin

  let tables = $state<TableSummary[]>([])
  let table = $state('')
  let lang = $state<Lang>('curl')
  let topic = $state<'tables' | 'auth'>('tables')
  let snippets = $state<Snippet[]>([])

  onMount(async () => {
    try {
      tables = (await api.get<{ tables: TableSummary[] }>('/tables')).tables
      table = tables.find((t) => t.kind === 'table')?.name ?? tables[0]?.name ?? ''
    } catch {
      tables = []
    }
  })

  // Examples for the chosen table, with its real columns (type, DEFAULT, generated).
  $effect(() => {
    if (topic === 'auth') {
      snippets = authSnippets(base)
      return
    }
    if (!table) {
      snippets = []
      return
    }
    const controller = new AbortController()
    api
      .get<TableData>(`/tables/${enc(table)}?size=1`, { signal: controller.signal })
      .then((data) => {
        snippets = tableSnippets(
          base,
          table,
          data.table.columns.map((c) => ({
            name: c.name,
            type: c.full_type,
            has_default: c.has_default,
            generated: c.generated,
            nullable: c.nullable,
          })),
        )
      })
      .catch((e) => {
        if (!isAbort(e)) snippets = tableSnippets(base, table, [])
      })
    return () => controller.abort()
  })

  const endpoints = [
    { id: 'rest', label: 'REST', path: '/rest/v1/' },
    { id: 'auth', label: 'Auth', path: '/auth/v1/' },
    { id: 'jwks', label: 'JWKS', path: '/auth/v1/.well-known/jwks.json' },
  ] as const

  const roles = ['anon', 'authenticated', 'service_role'] as const

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

  <section class="-mt-2 grid gap-4">
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

  <section class="grid gap-4">
    <div>
      <h2 class="text-base font-semibold">{t('connect.caller')}</h2>
      <p class="mt-1 text-sm text-muted-foreground">{t('connect.callerHint')}</p>
    </div>
    <div class="grid gap-3 md:grid-cols-3">
      {#each roles as role (role)}
        <div class="rounded-lg border bg-card p-4">
          <p class="text-sm font-medium">{t(`connect.roles.${role}.title`)}</p>
          <code class="text-xs text-muted-foreground">{role}</code>
          <p class="mt-2 text-sm text-muted-foreground">{t(`connect.roles.${role}.text`)}</p>
          {#if role === 'service_role'}
            <p class="mt-2 text-sm text-warning">{t('connect.neverInBrowser')}</p>
          {/if}
        </div>
      {/each}
    </div>
  </section>

  <ServiceTokenCard />

  <section class="grid gap-4">
    <h2 class="text-base font-semibold">{t('connect.examples')}</h2>
    <div class="flex flex-wrap items-center gap-3">
      <nav class="flex h-9 items-center gap-1 rounded-md bg-muted p-1" aria-label={t('connect.topic')}>
        <button type="button" class={tab(topic === 'tables')} aria-pressed={topic === 'tables'} onclick={() => (topic = 'tables')}
          >{t('connect.topics.tables')}</button
        >
        <button type="button" class={tab(topic === 'auth')} aria-pressed={topic === 'auth'} onclick={() => (topic = 'auth')}
          >{t('connect.topics.auth')}</button
        >
      </nav>
      {#if topic === 'tables' && tables.length}
        <Select.Root type="single" bind:value={table}>
          <Select.Trigger class="w-52 font-mono text-xs" aria-label={t('connect.table')}>{table}</Select.Trigger>
          <Select.Content>
            {#each tables as t (t.name)}
              <Select.Item value={t.name} class="font-mono text-xs">{t.name}</Select.Item>
            {/each}
          </Select.Content>
        </Select.Root>
      {/if}
      <nav class="flex h-9 items-center gap-1 rounded-md bg-muted p-1 sm:ml-auto" aria-label={t('connect.language')}>
        <button type="button" class={tab(lang === 'curl')} aria-pressed={lang === 'curl'} onclick={() => (lang = 'curl')}
          >curl</button
        >
        <button type="button" class={tab(lang === 'js')} aria-pressed={lang === 'js'} onclick={() => (lang = 'js')}
          >JavaScript</button
        >
      </nav>
    </div>

    {#if topic === 'tables' && !tables.length}
      <p class="rounded-lg border border-dashed px-4 py-8 text-center text-sm text-muted-foreground">
        {t('connect.noTables')}
      </p>
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
</div>
