<script lang="ts">
  import * as Card from '$lib/components/ui/card'
  import * as Table from '$lib/components/ui/table'
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu'
  import { Button } from '$lib/components/ui/button'
  import { Input } from '$lib/components/ui/input'
  import { Skeleton } from '$lib/components/ui/skeleton'
  import Search from '@lucide/svelte/icons/search'
  import Ellipsis from '@lucide/svelte/icons/ellipsis'
  import LogOut from '@lucide/svelte/icons/log-out'
  import Trash2 from '@lucide/svelte/icons/trash-2'
  import Copy from '@lucide/svelte/icons/copy'
  import ChevronLeft from '@lucide/svelte/icons/chevron-left'
  import ChevronRight from '@lucide/svelte/icons/chevron-right'
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

<div class="mx-auto max-w-6xl p-6 lg:p-8">
  <PageHeader
    title="Usuários"
    description="Usuários finais do /auth/v1. As senhas ficam em auth.users.encrypted_password (PHC argon2id) e nunca são exibidas."
  />

  <Card.Root class="gap-0 py-0">
    <div class="flex flex-wrap items-center gap-3 border-b px-4 py-3">
      <h2 class="text-sm font-semibold">{total} usuário(s)</h2>
      <div class="relative ml-auto w-full max-w-xs">
        <Search class="absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground" />
        <Input bind:value={query} oninput={onSearch} placeholder="Buscar por email" class="h-8 pl-8" />
      </div>
    </div>

    {#if users === null}
      <div class="space-y-2 p-4">{#each Array(4) as _, i (i)}<Skeleton class="h-10" />{/each}</div>
    {:else if users.length === 0}
      <p class="py-12 text-center text-sm text-muted-foreground">Nenhum usuário encontrado.</p>
    {:else}
      <Table.Root>
        <Table.Header>
          <Table.Row class="hover:bg-transparent">
            <Table.Head class="pl-4">Usuário</Table.Head>
            <Table.Head>Criado em</Table.Head>
            <Table.Head>Último login</Table.Head>
            <Table.Head>Sessões ativas</Table.Head>
            <Table.Head class="w-12"></Table.Head>
          </Table.Row>
        </Table.Header>
        <Table.Body>
          {#each users as user (user.id)}
            <Table.Row>
              <Table.Cell class="pl-4">
                <div class="flex items-center gap-3">
                  <span class="grid size-8 shrink-0 place-items-center rounded-full bg-primary/15 text-xs font-semibold text-primary">
                    {user.email[0]?.toUpperCase()}
                  </span>
                  <div class="min-w-0">
                    <p class="truncate font-medium">{user.email}</p>
                    <p class="truncate font-mono text-[11px] text-muted-foreground">{user.id}</p>
                  </div>
                </div>
              </Table.Cell>
              <Table.Cell class="text-muted-foreground">{when(user.created_at)}</Table.Cell>
              <Table.Cell class="text-muted-foreground">{when(user.last_sign_in_at)}</Table.Cell>
              <Table.Cell>
                <span class={['font-mono text-xs', user.sessions > 0 ? 'text-primary' : 'text-muted-foreground']}>{user.sessions}</span>
              </Table.Cell>
              <Table.Cell class="pr-4 text-right">
                <DropdownMenu.Root>
                  <DropdownMenu.Trigger>
                    {#snippet child({ props })}
                      <Button variant="ghost" size="icon-sm" aria-label="Ações" {...props}><Ellipsis /></Button>
                    {/snippet}
                  </DropdownMenu.Trigger>
                  <DropdownMenu.Content align="end" class="w-52">
                    <DropdownMenu.Item
                      onclick={() => {
                        navigator.clipboard.writeText(user.id)
                        toast.success('ID copiado')
                      }}><Copy />Copiar ID</DropdownMenu.Item
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
    {/if}
  </Card.Root>

  {#if page > 0 || hasNext}
    <div class="mt-4 flex justify-end gap-2">
      <Button variant="outline" size="sm" disabled={page === 0} onclick={() => page--}><ChevronLeft />Anterior</Button>
      <Button variant="outline" size="sm" disabled={!hasNext} onclick={() => page++}>Próxima<ChevronRight /></Button>
    </div>
  {/if}
</div>

{#if target}
  <ConfirmDialog
    bind:open={confirmOpen}
    title={target.action === 'revoke' ? 'Encerrar todas as sessões?' : 'Apagar este usuário?'}
    description={target.action === 'revoke'
      ? `${target.user.email} precisará entrar de novo. Os JWTs já emitidos valem até expirar.`
      : `${target.user.email} será apagado, com as sessões em cascata. Esta ação não pode ser desfeita.`}
    confirmLabel={target.action === 'revoke' ? 'Encerrar sessões' : 'Apagar'}
    destructive={target.action === 'delete'}
    onconfirm={confirm}
  />
{/if}
