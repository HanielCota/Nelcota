<script lang="ts">
  import * as Table from '$lib/components/ui/table'
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu'
  import { Button } from '$lib/components/ui/button'
  import { Input } from '$lib/components/ui/input'
  import { Skeleton } from '$lib/components/ui/skeleton'
  import Ellipsis from '@lucide/svelte/icons/ellipsis'
  import Search from '@lucide/svelte/icons/search'
  import UsersIcon from '@lucide/svelte/icons/users'
  import { toast } from 'svelte-sonner'
  import PageHeader from '$lib/components/app/PageHeader.svelte'
  import ConfirmDialog from '$lib/components/app/ConfirmDialog.svelte'
  import { api, enc } from '$lib/api'
  import type { User } from '$lib/types'

  let users = $state<User[] | null>(null)
  let total = $state(0)
  let page = $state(0)
  let hasNext = $state(false)
  let query = $state('')
  let target = $state<{ user: User; action: 'revoke' | 'delete' } | null>(null)
  let confirmOpen = $state(false)

  async function load() {
    try {
      const params = new URLSearchParams({ page: String(page) })
      if (query.trim()) params.set('q', query.trim())
      const data = await api.get<{ users: User[]; total: number; has_next: boolean }>(`/users?${params}`)
      users = data.users
      total = data.total
      hasNext = data.has_next
    } catch (e) {
      toast.error((e as Error).message)
    }
  }

  $effect(() => {
    void page
    load()
  })

  let debounce: ReturnType<typeof setTimeout>
  function onSearch() {
    clearTimeout(debounce)
    debounce = setTimeout(() => {
      page = 0
      load()
    }, 250)
  }

  function ask(user: User, action: 'revoke' | 'delete') {
    target = { user, action }
    confirmOpen = true
  }

  async function confirm() {
    if (!target) return
    const { user, action } = target
    try {
      const res =
        action === 'revoke'
          ? await api.post<{ message: string }>(`/users/${enc(user.id)}/revoke`)
          : await api.delete<{ message: string }>(`/users/${enc(user.id)}`)
      toast.success(res.message)
      await load()
    } catch (e) {
      toast.error((e as Error).message)
    }
  }

  const date = new Intl.DateTimeFormat('pt-BR', { dateStyle: 'short', timeStyle: 'short' })
  const when = (value: string | null) => (value ? date.format(new Date(value)) : '—')
</script>

<div class="mx-auto max-w-6xl px-6 py-10 lg:px-10">
  <PageHeader
    title="Usuários"
    description={users ? `${total} ${total === 1 ? 'usuário cadastrado' : 'usuários cadastrados'} em auth.users.` : 'Contas de auth.users.'}
  >
    {#snippet actions()}
      <div class="relative">
        <Search class="absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground" />
        <Input bind:value={query} oninput={onSearch} placeholder="Buscar por email" class="h-8 w-64 bg-card pl-8" />
      </div>
    {/snippet}
  </PageHeader>

  {#if users === null}
    <Skeleton class="h-64 rounded-lg" />
  {:else if users.length === 0}
    <div class="rounded-lg border border-dashed px-6 py-12 text-center">
      <UsersIcon class="mx-auto size-6 text-muted-foreground" strokeWidth={1.4} />
      <p class="mt-3 text-sm font-medium">Nenhum usuário</p>
      <p class="mt-1 text-sm font-light text-muted-foreground">
        {query.trim() ? 'Nada encontrado para essa busca.' : 'Os cadastros feitos pela API aparecem aqui.'}
      </p>
    </div>
  {:else}
    <div class="overflow-hidden rounded-lg border bg-card">
      <Table.Root>
        <Table.Header>
          <Table.Row class="hover:bg-transparent">
            <Table.Head>Email</Table.Head>
            <Table.Head>Criado</Table.Head>
            <Table.Head>Último login</Table.Head>
            <Table.Head class="text-right">Sessões</Table.Head>
            <Table.Head class="w-10"></Table.Head>
          </Table.Row>
        </Table.Header>
        <Table.Body>
          {#each users as user (user.id)}
            <Table.Row>
              <Table.Cell>
                <div class="flex items-center gap-3">
                  <span
                    class="grid size-8 shrink-0 place-items-center rounded-full border border-border-strong bg-muted text-xs font-medium"
                    >{user.email.charAt(0).toUpperCase()}</span
                  >
                  <div class="min-w-0">
                    <p class="font-medium">{user.email}</p>
                    <p class="font-mono text-2xs text-muted-foreground">{user.id}</p>
                  </div>
                </div>
              </Table.Cell>
              <Table.Cell class="text-muted-foreground">{when(user.created_at)}</Table.Cell>
              <Table.Cell class="text-muted-foreground">{when(user.last_sign_in_at)}</Table.Cell>
              <Table.Cell class="text-right font-mono text-xs tabular-nums">{user.sessions}</Table.Cell>
              <Table.Cell class="text-right">
                <DropdownMenu.Root>
                  <DropdownMenu.Trigger>
                    {#snippet child({ props })}
                      <Button variant="ghost" size="icon-sm" aria-label="Ações" {...props}><Ellipsis /></Button>
                    {/snippet}
                  </DropdownMenu.Trigger>
                  <DropdownMenu.Content align="end" class="w-48">
                    <DropdownMenu.Item
                      onclick={() => {
                        navigator.clipboard.writeText(user.id)
                        toast.success('ID copiado')
                      }}>Copiar ID</DropdownMenu.Item
                    >
                    <DropdownMenu.Item onclick={() => ask(user, 'revoke')}>Encerrar sessões</DropdownMenu.Item>
                    <DropdownMenu.Separator />
                    <DropdownMenu.Item variant="destructive" onclick={() => ask(user, 'delete')}>
                      Apagar usuário
                    </DropdownMenu.Item>
                  </DropdownMenu.Content>
                </DropdownMenu.Root>
              </Table.Cell>
            </Table.Row>
          {/each}
        </Table.Body>
      </Table.Root>
    </div>
  {/if}

  {#if page > 0 || hasNext}
    <div class="mt-4 flex items-center justify-end gap-2 text-xs text-muted-foreground">
      <span class="mr-1">Página {page + 1}</span>
      <Button variant="outline" size="sm" disabled={page === 0} onclick={() => page--}>Anterior</Button>
      <Button variant="outline" size="sm" disabled={!hasNext} onclick={() => page++}>Próxima</Button>
    </div>
  {/if}
</div>

{#if target}
  <ConfirmDialog
    bind:open={confirmOpen}
    title={target.action === 'revoke' ? 'Encerrar as sessões?' : 'Apagar o usuário?'}
    description={target.action === 'revoke'
      ? `${target.user.email} vai precisar entrar de novo. JWTs já emitidos valem até expirar.`
      : `${target.user.email} e as sessões dele serão apagados. Não dá para desfazer.`}
    confirmLabel={target.action === 'revoke' ? 'Encerrar sessões' : 'Apagar'}
    destructive={target.action === 'delete'}
    onconfirm={confirm}
  />
{/if}
