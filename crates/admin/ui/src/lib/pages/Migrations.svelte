<script lang="ts">
  import { onMount } from 'svelte'
  import * as Table from '$lib/components/ui/table'
  import * as Dialog from '$lib/components/ui/dialog'
  import { Button } from '$lib/components/ui/button'
  import { Input } from '$lib/components/ui/input'
  import { Label } from '$lib/components/ui/label'
  import { Skeleton } from '$lib/components/ui/skeleton'
  import { toast } from 'svelte-sonner'
  import PageHeader from '$lib/components/app/PageHeader.svelte'
  import EmptyState from '$lib/components/app/EmptyState.svelte'
  import { api } from '$lib/api'
  import { downloadText } from '$lib/download'
  import type { ExportedMigration, MigrationsData } from '$lib/types'

  let data = $state<MigrationsData | null>(null)
  let error = $state('')

  async function load() {
    try {
      data = await api.get<MigrationsData>('/migrations')
      error = ''
    } catch (e) {
      error = (e as Error).message
    }
  }

  onMount(load)

  // Diálogo "Gerar migração".
  let dialogOpen = $state(false)
  let name = $state('alteracoes_do_painel')
  let saving = $state(false)
  // Última migração gerada nesta visita, para lembrar o próximo passo.
  let generated = $state<ExportedMigration | null>(null)

  const validName = $derived(/^[a-z0-9][a-z0-9_]{0,59}$/.test(name))

  async function generate(event: SubmitEvent) {
    event.preventDefault()
    if (!validName) return
    saving = true
    try {
      const result = await api.post<ExportedMigration>('/migrations', { name })
      downloadText(result.filename, result.sql, 'application/sql')
      generated = result
      dialogOpen = false
      toast.success(result.message)
      await load()
    } catch (e) {
      toast.error((e as Error).message)
    } finally {
      saving = false
    }
  }

  const when = new Intl.DateTimeFormat('pt-BR', { dateStyle: 'short', timeStyle: 'short' })
  const date = (value: string | null) => {
    if (!value) return '—'
    const parsed = new Date(value)
    return Number.isNaN(parsed.getTime()) ? value : when.format(parsed)
  }

  type Row = MigrationsData['migrations'][number]
  // Situação de cada migração; cor só para o que pede ação (D55).
  function status(m: Row): { label: string; warn: boolean } {
    if (!m.applied_on) return { label: 'Não aplicada neste banco', warn: true }
    if (m.in_folder === false) return { label: 'Aplicada, mas fora da pasta', warn: true }
    return { label: 'Aplicada', warn: false }
  }
</script>

<div class="mx-auto max-w-6xl px-4 py-8 sm:px-6 lg:px-10 lg:py-10">
  <PageHeader title="Migrações" description="Arquivos de migrations/ e alterações de schema feitas pelo painel." />

  {#if error}
    <p class="text-sm text-destructive">
      {error} <button type="button" class="ml-1 underline underline-offset-2" onclick={load}>Tentar de novo</button>
    </p>
  {:else if !data}
    <Skeleton class="h-40 rounded-lg" />
    <Skeleton class="mt-8 h-64 rounded-lg" />
  {:else}
    <section class="grid gap-3">
      <div class="flex flex-wrap items-end justify-between gap-3">
        <div>
          <h2 class="text-base font-semibold">Alterações do painel fora das migrações</h2>
          <p class="mt-0.5 text-sm text-muted-foreground">
            Tabelas, colunas e policies criadas aqui ainda não existem em outro ambiente até virarem arquivo.
          </p>
        </div>
        {#if data.pending.length}
          <Button onclick={() => (dialogOpen = true)}>Gerar migração</Button>
        {/if}
      </div>

      {#if generated}
        <div class="rounded-lg border px-4 py-3 text-sm">
          <p>
            <span class="font-mono">{generated.filename}</span> foi baixada. Coloque em
            <span class="font-mono">migrations/</span> e faça commit: neste banco ela já consta como aplicada, e o
            <span class="font-mono">nelcota migrate</span> aplica nos outros ambientes.
          </p>
        </div>
      {/if}

      {#if data.pending.length === 0}
        <EmptyState
          class="rounded-lg border"
          title="Nada pendente"
          description="O que você mudar pelo painel aparece aqui até ser exportado como migração."
        />
      {:else}
        <ol class="divide-y rounded-lg border bg-card">
          {#each data.pending as change (change.id)}
            <li class="px-4 py-3">
              <div class="flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1">
                <p class="text-sm font-medium">{change.summary}</p>
                <p class="text-xs text-muted-foreground">{date(change.applied_at)}</p>
              </div>
              <details class="mt-1 text-sm">
                <summary class="cursor-pointer text-muted-foreground hover:text-foreground">
                  SQL ({change.statements.length}
                  {change.statements.length === 1 ? 'comando' : 'comandos'})
                </summary>
                <pre class="mt-2 overflow-x-auto rounded-md bg-muted/50 px-3 py-2 font-mono text-xs leading-relaxed">{change.statements
                    .map((s) => s.trim().replace(/;$/, '') + ';')
                    .join('\n')}</pre>
              </details>
            </li>
          {/each}
        </ol>
      {/if}
    </section>

    <section class="mt-10 grid gap-3">
      <h2 class="text-base font-semibold">Migrações</h2>
      {#if data.migrations.length === 0}
        <EmptyState
          class="rounded-lg border"
          title="Nenhuma migração ainda"
          description="Arquivos V1__nome.sql em migrations/, aplicados com nelcota migrate, aparecem aqui."
        />
      {:else}
        <div class="overflow-hidden rounded-lg border bg-card">
          <Table.Root>
            <Table.Header>
              <Table.Row class="hover:bg-transparent">
                <Table.Head class="w-20">Versão</Table.Head>
                <Table.Head>Nome</Table.Head>
                <Table.Head>Situação</Table.Head>
                <Table.Head>Aplicada em</Table.Head>
                <Table.Head class="w-28"></Table.Head>
              </Table.Row>
            </Table.Header>
            <Table.Body>
              {#each data.migrations as migration (migration.version)}
                {@const situation = status(migration)}
                <Table.Row>
                  <Table.Cell class="font-mono text-xs tabular-nums">V{migration.version}</Table.Cell>
                  <Table.Cell>
                    <span class="font-mono text-xs">{migration.name}</span>
                    {#if migration.from_panel}<span class="ml-2 text-xs text-muted-foreground">gerada pelo painel</span>{/if}
                  </Table.Cell>
                  <Table.Cell class={situation.warn ? 'text-warning' : 'text-muted-foreground'}>{situation.label}</Table.Cell>
                  <Table.Cell class="text-muted-foreground">{date(migration.applied_on)}</Table.Cell>
                  <Table.Cell class="text-right">
                    {#if migration.from_panel}
                      <Button
                        variant="ghost"
                        size="sm"
                        href={`/admin/api/migrations/${migration.version}/file`}
                        download>Baixar</Button
                      >
                    {/if}
                  </Table.Cell>
                </Table.Row>
              {/each}
            </Table.Body>
          </Table.Root>
        </div>
      {/if}
      <p class="text-sm text-muted-foreground">
        {#if data.folder}
          Pasta lida: <span class="font-mono">{data.folder}</span>.
        {:else}
          A pasta migrations/ não está acessível a este servidor: a numeração considera só o banco.
        {/if}
      </p>
    </section>
  {/if}
</div>

<Dialog.Root bind:open={dialogOpen}>
  <Dialog.Content class="sm:max-w-md">
    <form class="grid gap-5" onsubmit={generate}>
      <Dialog.Header>
        <Dialog.Title>Gerar migração</Dialog.Title>
        <Dialog.Description>
          Junta as {data?.pending.length ?? 0} alterações pendentes num arquivo e o registra como aplicado neste banco.
        </Dialog.Description>
      </Dialog.Header>
      <div class="grid gap-2">
        <Label for="migration-name">Nome</Label>
        <Input id="migration-name" bind:value={name} maxlength={60} autocomplete="off" aria-invalid={!validName} />
        <p class="text-sm text-muted-foreground">
          {#if validName}
            Arquivo: <span class="font-mono">V{data?.next_version}__{name}.sql</span>
          {:else}
            Letras minúsculas, números e _ (ex.: criar_pedidos).
          {/if}
        </p>
      </div>
      <Dialog.Footer>
        <Button variant="outline" onclick={() => (dialogOpen = false)}>Cancelar</Button>
        <Button type="submit" disabled={!validName || saving}>{saving ? 'Gerando…' : 'Gerar e baixar'}</Button>
      </Dialog.Footer>
    </form>
  </Dialog.Content>
</Dialog.Root>
