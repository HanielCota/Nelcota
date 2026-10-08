<script lang="ts">
  import { untrack } from 'svelte'
  import * as Table from '$lib/components/ui/table'
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu'
  import { Button } from '$lib/components/ui/button'
  import { Input } from '$lib/components/ui/input'
  import { Skeleton } from '$lib/components/ui/skeleton'
  import Ellipsis from '@lucide/svelte/icons/ellipsis'
  import Search from '@lucide/svelte/icons/search'
  import UserPlus from '@lucide/svelte/icons/user-plus'
  import UsersIcon from '@lucide/svelte/icons/users'
  import Copy from '@lucide/svelte/icons/copy'
  import KeyRound from '@lucide/svelte/icons/key-round'
  import LogOut from '@lucide/svelte/icons/log-out'
  import MailCheck from '@lucide/svelte/icons/mail-check'
  import Trash2 from '@lucide/svelte/icons/trash-2'
  import { Badge } from '$lib/components/ui/badge'
  import { toast } from 'svelte-sonner'
  import PageHeader from '$lib/components/shared/PageHeader.svelte'
  import EmptyState from '$lib/components/shared/EmptyState.svelte'
  import LoadError from '$lib/components/shared/LoadError.svelte'
  import ConfirmDialog from '$lib/components/shared/ConfirmDialog.svelte'
  import CreateUserDialog from '$lib/features/users/components/CreateUserDialog.svelte'
  import SetPasswordDialog from '$lib/features/users/components/SetPasswordDialog.svelte'
  import { api, enc } from '$lib/api'
  import { href, navigate, route } from '$lib/router.svelte'
  import { copyText } from '$lib/clipboard'
  import { RemoteResource } from '$lib/remote-resource.svelte'
  import type { UsersResponse, User } from '$lib/types'
  import { errorMessage, intlLocale, t } from '$lib/i18n/index.svelte'

  const resource = new RemoteResource<UsersResponse>()
  const users = $derived(resource.data?.users ?? null)
  const total = $derived(resource.data?.total ?? 0)
  const page = $derived(Math.max(0, Number.parseInt(route.query.get('page') ?? '0') || 0))
  const appliedQuery = $derived(route.query.get('q') ?? '')
  const hasNext = $derived(resource.data?.has_next ?? false)
  let query = $state('')
  const loading = $derived(resource.loading)
  const error = $derived(resource.error ? errorMessage(resource.error) : '')

  function go(nextPage: number, nextQuery = appliedQuery, replace = false) {
    const params = new URLSearchParams()
    if (nextPage) params.set('page', String(nextPage))
    if (nextQuery) params.set('q', nextQuery)
    navigate(`/users${params.size ? `?${params}` : ''}`, replace)
  }
  let target = $state<{ user: User; action: 'revoke' | 'delete' } | null>(null)
  let confirmOpen = $state(false)
  let createOpen = $state(false)
  let passwordUser = $state<User | null>(null)
  let passwordOpen = $state(false)

  async function load() {
    const params = new URLSearchParams({ page: String(page) })
    if (appliedQuery) params.set('q', appliedQuery)
    await resource.load(signal => api.get<UsersResponse>(`/users?${params}`, { signal }))
  }

  $effect(() => {
    void [page, appliedQuery]
    untrack(load)
    return () => resource.cancel()
  })

  $effect(() => { query = appliedQuery })

  let debounce: ReturnType<typeof setTimeout>
  function onSearch() {
    clearTimeout(debounce)
    debounce = setTimeout(() => {
      go(0, query.trim(), true)
    }, 250)
  }

  $effect(() => () => clearTimeout(debounce))

  function ask(user: User, action: 'revoke' | 'delete') {
    target = { user, action }
    confirmOpen = true
  }

  async function confirmEmail(user: User) {
    try {
      await api.post(`/users/${enc(user.id)}/confirm`)
      toast.success(t('users.emailConfirmed', { email: user.email }))
      await load()
    } catch (e) {
      toast.error(errorMessage(e))
    }
  }

  async function confirm() {
    if (!target) return
    const { user, action } = target
    try {
      if (action === 'revoke') {
        await api.post(`/users/${enc(user.id)}/revoke`)
        toast.success(t('users.revoked', { email: user.email }))
      } else {
        await api.delete(`/users/${enc(user.id)}`)
        toast.success(t('users.deleted', { email: user.email }))
      }
      await load()
    } catch (e) {
      toast.error(errorMessage(e))
      throw e
    }
  }

  const date = $derived(new Intl.DateTimeFormat(intlLocale(), { dateStyle: 'short', timeStyle: 'short' }))
  const when = (value: string | null) => (value ? date.format(new Date(value)) : '—')
</script>

{#snippet unconfirmed(user: User)}
  {#if !user.email_confirmed_at}
    <Badge variant="outline" class="ml-2 align-middle font-normal text-muted-foreground" title={t('users.unconfirmedHint')}>{t('users.unconfirmed')}</Badge>
  {/if}
{/snippet}

{#snippet userActions(user: User)}
  <DropdownMenu.Root>
    <DropdownMenu.Trigger>
      {#snippet child({ props })}
        <Button variant="ghost" size="icon-sm" aria-label={t('common.actionsFor', { name: user.email })} {...props}><Ellipsis aria-hidden="true" /></Button>
      {/snippet}
    </DropdownMenu.Trigger>
    <DropdownMenu.Content align="end" class="w-56">
      <DropdownMenu.Group>
        <DropdownMenu.Item onclick={() => copyText(user.id, t('users.idCopied'))}><Copy aria-hidden="true" />{t('users.copyId')}</DropdownMenu.Item>
        <DropdownMenu.Item onclick={() => { passwordUser = user; passwordOpen = true }}><KeyRound aria-hidden="true" />{t('users.resetPassword')}</DropdownMenu.Item>
        {#if !user.email_confirmed_at}
          <DropdownMenu.Item onclick={() => confirmEmail(user)}><MailCheck aria-hidden="true" />{t('users.confirmEmail')}</DropdownMenu.Item>
        {/if}
        <DropdownMenu.Item onclick={() => ask(user, 'revoke')}><LogOut aria-hidden="true" />{t('users.revokeSessions')}</DropdownMenu.Item>
      </DropdownMenu.Group>
      <DropdownMenu.Separator />
      <DropdownMenu.Group><DropdownMenu.Item variant="destructive" onclick={() => ask(user, 'delete')}><Trash2 aria-hidden="true" />{t('users.delete')}</DropdownMenu.Item></DropdownMenu.Group>
    </DropdownMenu.Content>
  </DropdownMenu.Root>
{/snippet}

<div class="mx-auto max-w-6xl px-4 py-8 sm:px-6 lg:px-10 lg:py-10">
  <PageHeader
    title={t('users.title')}
    description={users ? t('users.count', { count: total }) : undefined}
  >
    {#snippet actions()}
      <div class="relative w-full sm:w-72">
        <Search class="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground" />
        <Input
          type="search"
          bind:value={query}
          oninput={onSearch}
          placeholder={t('users.searchPlaceholder')}
          aria-label={t('users.searchLabel')}
          class="pl-9"
        />
      </div>
      <Button onclick={() => (createOpen = true)}><UserPlus />{t('users.new')}</Button>
    {/snippet}
  </PageHeader>

  {#if error}<LoadError message={error} onretry={load} busy={loading} />{/if}
  {#if users === null}
    {#if loading}<Skeleton class="h-64 rounded-lg" />{/if}
  {:else if users.length === 0}
    <EmptyState
      class="rounded-lg border"
      icon={appliedQuery ? Search : UsersIcon}
      title={appliedQuery ? t('users.noResults') : t('users.empty')}
      description={appliedQuery
        ? t('users.noResultsFor', { query: appliedQuery })
        : t('users.emptyDescription')}
    >
      {#snippet actions()}
        {#if appliedQuery}
          <Button
            variant="outline"
            onclick={() => {
              go(0, '')
            }}>{t('users.clearSearch')}</Button
          >
        {:else}
          <Button variant="outline" onclick={() => (createOpen = true)}><UserPlus data-icon="inline-start" aria-hidden="true" />{t('users.new')}</Button>
        {/if}
      {/snippet}
    </EmptyState>
  {:else}
    <div class="grid gap-3 md:hidden" aria-busy={loading}>
      {#each users as user (user.id)}
        <article class="min-w-0 rounded-lg border bg-card p-4">
          <div class="flex min-w-0 items-start justify-between gap-2">
            <div class="min-w-0"><p class="break-all font-medium">{user.email}{@render unconfirmed(user)}</p><p class="mt-1 truncate font-mono text-xs text-muted-foreground" title={user.id}>{user.id}</p></div>
            {@render userActions(user)}
          </div>
          <dl class="mt-4 grid grid-cols-[auto_minmax(0,1fr)] gap-x-3 gap-y-2 text-xs">
            <dt class="text-muted-foreground">{t('users.columns.lastSignIn')}</dt><dd class="text-right">{when(user.last_sign_in_at)}</dd>
            <dt class="text-muted-foreground">{t('users.columns.created')}</dt><dd class="text-right">{when(user.created_at)}</dd>
            <dt class="text-muted-foreground">{t('users.columns.sessions')}</dt><dd class="text-right"><Badge variant="secondary">{user.sessions}</Badge></dd>
          </dl>
        </article>
      {/each}
    </div>
    <div class="hidden overflow-hidden rounded-lg border bg-card md:block" aria-busy={loading}>
      <Table.Root>
        <Table.Header>
          <Table.Row class="hover:bg-transparent">
            <Table.Head>{t('users.columns.email')}</Table.Head>
            <Table.Head>{t('users.columns.created')}</Table.Head>
            <Table.Head>{t('users.columns.lastSignIn')}</Table.Head>
            <Table.Head class="text-right">{t('users.columns.sessions')}</Table.Head>
            <Table.Head class="w-12"><span class="sr-only">{t('common.actions')}</span></Table.Head>
          </Table.Row>
        </Table.Header>
        <Table.Body>
          {#each users as user (user.id)}
            <Table.Row>
              <Table.Cell>
                <div class="flex items-center gap-3">
                  <span
                    class="grid size-8 shrink-0 place-items-center rounded-full border border-border-strong bg-muted text-xs font-medium"
                    aria-hidden="true">{user.email.charAt(0).toUpperCase()}</span
                  >
                  <div class="min-w-0">
                    <p class="font-medium">{user.email}{@render unconfirmed(user)}</p>
                    <p class="max-w-60 truncate font-mono text-xs text-muted-foreground" title={user.id}>{user.id}</p>
                  </div>
                </div>
              </Table.Cell>
              <Table.Cell class="text-muted-foreground">{when(user.created_at)}</Table.Cell>
              <Table.Cell class="text-muted-foreground">{when(user.last_sign_in_at)}</Table.Cell>
              <Table.Cell class="text-right font-mono text-xs tabular-nums">{user.sessions}</Table.Cell>
              <Table.Cell class="text-right">
                {@render userActions(user)}
              </Table.Cell>
            </Table.Row>
          {/each}
        </Table.Body>
      </Table.Root>
    </div>
  {/if}

  {#if page > 0 || hasNext}
    <nav class="mt-4 flex items-center justify-end gap-2" aria-label={t('users.pagination')}>
      <span class="mr-1 text-sm text-muted-foreground">{t('common.page', { page: page + 1 })}</span>
      <Button variant="outline" size="sm" disabled={loading || page === 0} href={href(`/users?${new URLSearchParams({ page: String(page - 1), ...(appliedQuery && { q: appliedQuery }) })}`)}>{t('common.previous')}</Button>
      <Button variant="outline" size="sm" disabled={loading || !hasNext} onclick={() => go(page + 1)}>{t('common.next')}</Button>
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
    title={target.action === 'revoke' ? t('users.confirmRevoke.title') : t('users.confirmDelete.title')}
    description={target.action === 'revoke'
      ? t('users.confirmRevoke.description', { email: target.user.email })
      : t('users.confirmDelete.description', { email: target.user.email })}
    confirmLabel={target.action === 'revoke' ? t('users.revokeSessions') : t('common.delete')}
    destructive={target.action === 'delete'}
    onconfirm={confirm}
  />
{/if}
