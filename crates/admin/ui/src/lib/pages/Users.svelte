<script lang="ts">
  import * as Table from '$lib/components/ui/table'
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu'
  import { Button } from '$lib/components/ui/button'
  import { Input } from '$lib/components/ui/input'
  import { Skeleton } from '$lib/components/ui/skeleton'
  import Ellipsis from '@lucide/svelte/icons/ellipsis'
  import Search from '@lucide/svelte/icons/search'
  import UsersIcon from '@lucide/svelte/icons/users'
  import UserPlus from '@lucide/svelte/icons/user-plus'
  import Copy from '@lucide/svelte/icons/copy'
  import KeyRound from '@lucide/svelte/icons/key-round'
  import LogOut from '@lucide/svelte/icons/log-out'
  import Trash2 from '@lucide/svelte/icons/trash-2'
  import ChevronLeft from '@lucide/svelte/icons/chevron-left'
  import ChevronRight from '@lucide/svelte/icons/chevron-right'
  import { toast } from 'svelte-sonner'
  import PageHeader from '$lib/components/app/PageHeader.svelte'
  import EmptyState from '$lib/components/app/EmptyState.svelte'
  import ConfirmDialog from '$lib/components/app/ConfirmDialog.svelte'
  import CreateUserDialog from '$lib/components/app/CreateUserDialog.svelte'
  import SetPasswordDialog from '$lib/components/app/SetPasswordDialog.svelte'
  import { api, enc } from '$lib/api'
  import type { User } from '$lib/types'

  let users = $state<User[] | null>(null)
  let total = $state(0)
  let page = $state(0)
  let hasNext = $state(false)
  let query = $state('')
  let target = $state<{ user: User; action: 'revoke' | 'delete' } | null>(null)
  let confirmOpen = $state(false)
  let createOpen = $state(false)
  let passwordUser = $state<User | null>(null)
  let passwordOpen = $state(false)

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

<div class="mx-auto max-w-6xl px-4 py-8 sm:px-6 lg:px-10 lg:py-12">
  <PageHeader
    title="Usuários"
    icon={UsersIcon}
    description={users ? `${total} ${total === 1 ? 'usuário cadastrado' : 'usuários cadastrados'} em auth.users.` : 'Contas de auth.users.'}
  >
    {#snippet actions()}
      <div class="relative w-full sm:w-72">
        <Search class="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground" />
        <Input
          type="search"
          bind:value={query}
          oninput={onSearch}
          placeholder="Buscar por email"
          aria-label="Buscar usuários por email"
          class="pl-9"
        />
      </div>
      <Button onclick={() => (createOpen = true)}><UserPlus />Novo usuário</Button>
    {/snippet}
  </PageHeader>

  {#if users === null}
    <Skeleton class="h-72 rounded-xl" />
  {:else if users.length === 0}
    <EmptyState
      icon={query.trim() ? Search : UsersIcon}
      title={query.trim() ? 'Nenhum resultado' : 'Nenhum usuário'}
      description={query.trim()
        ? `Nada encontrado para "${query.trim()}".`
        : 'Cadastros feitos pela API aparecem aqui, ou crie um agora.'}
    >
      {#snippet actions()}
        {#if query.trim()}
          <Button
            variant="outline"
            onclick={() => {
              query = ''
              page = 0
              load()
            }}>Limpar busca</Button
          >
        {:else}
          <Button onclick={() => (createOpen = true)}><UserPlus />Novo usuário</Button>
        {/if}
      {/snippet}
    </EmptyState>
  {:else}
    <div class="overflow-hidden rounded-xl border bg-card shadow-card">
      <Table.Root>
        <Table.Header>
          <Table.Row class="hover:bg-transparent">
            <Table.Head>Email</Table.Head>
            <Table.Head>Criado</Table.Head>
            <Table.Head>Último login</Table.Head>
            <Table.Head class="text-right">Sessões</Table.Head>
            <Table.Head class="w-12"><span class="sr-only">Ações</span></Table.Head>
          </Table.Row>
        </Table.Header>
        <Table.Body>
          {#each users as user (user.id)}
            <Table.Row>
              <Table.Cell>
                <div class="flex items-center gap-3">
                  <span
                    class="grid size-9 shrink-0 place-items-center rounded-full bg-brand-soft text-sm font-bold text-brand ring-1 ring-brand/25"
                    aria-hidden="true">{user.email.charAt(0).toUpperCase()}</span
                  >
                  <div class="min-w-0">
                    <p class="font-semibold">{user.email}</p>
                    <p class="font-mono text-2xs text-muted-foreground">{user.id}</p>
                  </div>
                </div>
              </Table.Cell>
              <Table.Cell class="text-muted-foreground">{when(user.created_at)}</Table.Cell>
              <Table.Cell class="text-muted-foreground">{when(user.last_sign_in_at)}</Table.Cell>
              <Table.Cell class="text-right">
                <span
                  class={[
                    'inline-flex min-w-7 justify-center rounded-full px-2 py-0.5 font-mono text-xs font-medium tabular-nums',
                    user.sessions > 0 ? 'bg-brand-soft text-brand' : 'bg-muted text-muted-foreground',
                  ]}>{user.sessions}</span
                >
              </Table.Cell>
              <Table.Cell class="text-right">
                <DropdownMenu.Root>
                  <DropdownMenu.Trigger>
                    {#snippet child({ props })}
                      <Button variant="ghost" size="icon-sm" aria-label="Ações" {...props}><Ellipsis /></Button>
                    {/snippet}
                  </DropdownMenu.Trigger>
                  <DropdownMenu.Content align="end" class="w-56">
                    <DropdownMenu.Item
                      onclick={() => {
                        navigator.clipboard.writeText(user.id)
                        toast.success('ID copiado')
                      }}><Copy />Copiar ID</DropdownMenu.Item
                    >
                    <DropdownMenu.Item
                      onclick={() => {
                        passwordUser = user
                        passwordOpen = true
                      }}><KeyRound />Redefinir senha…</DropdownMenu.Item
                    >
                    <DropdownMenu.Item onclick={() => ask(user, 'revoke')}><LogOut />Encerrar sessões</DropdownMenu.Item>
                    <DropdownMenu.Separator />
                    <DropdownMenu.Item variant="destructive" onclick={() => ask(user, 'delete')}>
                      <Trash2 />Apagar usuário
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
    <nav class="mt-5 flex flex-wrap items-center justify-between gap-3" aria-label="Paginação">
      <span class="text-sm text-muted-foreground">Página <span class="font-semibold text-foreground">{page + 1}</span></span>
      <div class="flex items-center gap-2">
        <Button variant="outline" size="sm" disabled={page === 0} onclick={() => page--}><ChevronLeft />Anterior</Button>
        <Button variant="outline" size="sm" disabled={!hasNext} onclick={() => page++}>Próxima<ChevronRight /></Button>
      </div>
    </nav>
  {/if}
</div>

<CreateUserDialog bind:open={createOpen} oncreated={load} />
{#if passwordUser}
  <SetPasswordDialog bind:open={passwordOpen} user={passwordUser} onsaved={load} />
{/if}

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
