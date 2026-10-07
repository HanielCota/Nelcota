<script lang="ts">
  import { onMount } from 'svelte'
  import * as Select from '$lib/components/ui/select'
  import Globe from '@lucide/svelte/icons/globe'
  import UserRound from '@lucide/svelte/icons/user-round'
  import Server from '@lucide/svelte/icons/server'
  import BookOpen from '@lucide/svelte/icons/book-open'
  import PageHeader from '$lib/components/app/PageHeader.svelte'
  import CodeBlock from '$lib/components/app/CodeBlock.svelte'
  import ServiceTokenCard from '$lib/components/app/ServiceTokenCard.svelte'
  import { api, enc, isAbort } from '$lib/api'
  import { authSnippets, tableSnippets, type Lang, type Snippet } from '$lib/snippets'
  import type { TableData, TableSummary } from '$lib/types'

  // O painel é servido pelo mesmo host da API.
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

  // Exemplos da tabela escolhida, com as colunas reais (tipo, DEFAULT, gerada).
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
    { label: 'REST', path: '/rest/v1/', hint: 'tabelas, views e funções (rpc) do schema exposto' },
    { label: 'Auth', path: '/auth/v1/', hint: 'cadastro, login, sessão e usuário atual' },
    { label: 'JWKS', path: '/auth/v1/.well-known/jwks.json', hint: 'chave pública para validar os JWTs' },
  ]

  const roles = [
    {
      icon: Globe,
      role: 'anon',
      title: 'Visitante',
      text: 'Request sem header Authorization. Vê o que os GRANTs e as policies liberam para anon.',
    },
    {
      icon: UserRound,
      role: 'authenticated',
      title: 'Usuário logado',
      text: 'Authorization: Bearer com o access_token do login. auth.uid() é o id dele nas policies.',
    },
    {
      icon: Server,
      role: 'service_role',
      title: 'Seu backend',
      text: 'Bearer com o token service_role. Ignora o RLS (mas precisa de GRANT na tabela): só em código que roda no servidor.',
    },
  ]

  const tab = (active: boolean) =>
    ['rounded px-2.5 py-1 text-xs transition-colors', active ? 'bg-card text-foreground shadow-xs' : 'text-muted-foreground hover:text-foreground']
</script>

<div class="mx-auto grid max-w-5xl gap-8 px-6 py-10 *:min-w-0 lg:px-10">
  <PageHeader title="API" description="Como o seu app conversa com este projeto." />

  <section class="grid gap-3 rounded-lg border bg-card p-5">
    <div class="flex flex-wrap items-center justify-between gap-2">
      <h2 class="text-sm font-medium">Endereço do projeto</h2>
      <a
        href="/rest/v1/"
        target="_blank"
        rel="noopener"
        class="inline-flex items-center gap-1.5 text-sm text-muted-foreground hover:text-brand"
        ><BookOpen class="size-4" />Documentação OpenAPI</a
      >
    </div>
    <CodeBlock code={base} label="Copiar endereço" />
    <dl class="grid gap-2 text-sm sm:grid-cols-[6rem_1fr]">
      {#each endpoints as endpoint (endpoint.path)}
        <dt class="font-medium">{endpoint.label}</dt>
        <dd class="flex flex-wrap gap-x-3">
          <code class="text-xs">{endpoint.path}</code>
          <span class="text-xs font-light text-muted-foreground">{endpoint.hint}</span>
        </dd>
      {/each}
    </dl>
  </section>

  <section class="grid gap-3">
    <h2 class="text-sm font-medium">Quem faz a chamada</h2>
    <div class="grid gap-3 md:grid-cols-3">
      {#each roles as item (item.role)}
        <div class="rounded-lg border bg-card p-4">
          <item.icon class="size-4 text-muted-foreground" strokeWidth={1.6} />
          <p class="mt-3 text-sm font-medium">{item.title}</p>
          <code class="text-xs text-brand">{item.role}</code>
          <p class="mt-2 text-xs leading-relaxed font-light text-muted-foreground">{item.text}</p>
        </div>
      {/each}
    </div>
    <p class="text-xs font-light text-muted-foreground">
      Não existe chave anon: quem não manda token já é anon. O acesso de cada role é definido por tabela, nos GRANTs
      (aba Estrutura) e nas policies.
    </p>
  </section>

  <ServiceTokenCard />

  <section class="grid gap-4">
    <div class="flex flex-wrap items-center gap-3">
      <h2 class="text-sm font-medium">Exemplos</h2>
      <nav class="flex items-center gap-0.5 rounded-md bg-muted p-0.5" aria-label="Assunto">
        <button type="button" class={tab(topic === 'tables')} onclick={() => (topic = 'tables')}>Tabelas</button>
        <button type="button" class={tab(topic === 'auth')} onclick={() => (topic = 'auth')}>Autenticação</button>
      </nav>
      {#if topic === 'tables' && tables.length}
        <Select.Root type="single" bind:value={table}>
          <Select.Trigger size="sm" class="w-48 font-mono text-xs" aria-label="Tabela">{table}</Select.Trigger>
          <Select.Content>
            {#each tables as t (t.name)}
              <Select.Item value={t.name} class="font-mono text-xs">{t.name}</Select.Item>
            {/each}
          </Select.Content>
        </Select.Root>
      {/if}
      <nav class="ml-auto flex items-center gap-0.5 rounded-md bg-muted p-0.5" aria-label="Linguagem">
        <button type="button" class={tab(lang === 'curl')} onclick={() => (lang = 'curl')}>curl</button>
        <button type="button" class={tab(lang === 'js')} onclick={() => (lang = 'js')}>JavaScript</button>
      </nav>
    </div>

    {#if topic === 'tables' && !tables.length}
      <p class="rounded-lg border border-dashed px-4 py-8 text-center text-sm font-light text-muted-foreground">
        Crie uma tabela para ver exemplos com as colunas dela.
      </p>
    {:else}
      <div class="grid gap-5 *:min-w-0">
        {#each snippets as snippet (snippet.id)}
          <article class="grid gap-2 *:min-w-0">
            <div>
              <h3 class="text-sm">{snippet.label}</h3>
              <p class="text-xs font-light text-muted-foreground">{snippet.description}</p>
            </div>
            <CodeBlock code={snippet.code[lang]} />
          </article>
        {/each}
      </div>
    {/if}
    <p class="text-xs font-light text-muted-foreground">
      Tipos TypeScript das tabelas: <code class="text-foreground">nelcota types -o database.ts</code>
    </p>
  </section>
</div>
