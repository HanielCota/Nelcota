<script lang="ts">
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu'
  import { mode, toggleMode } from 'mode-watcher'
  import Sun from '@lucide/svelte/icons/sun'
  import Moon from '@lucide/svelte/icons/moon'
  import LogOut from '@lucide/svelte/icons/log-out'
  import ChevronsUpDown from '@lucide/svelte/icons/chevrons-up-down'
  import ImageUp from '@lucide/svelte/icons/image-up'
  import Avatar from './Avatar.svelte'
  import AvatarDialog from './AvatarDialog.svelte'
  import { logout } from '$lib/auth'
  import { loadProfile, profile } from '$lib/profile.svelte'
  import { session } from '$lib/session.svelte'

  let {
    open = $bindable(false),
    reveal = '',
  }: {
    open?: boolean
    /** Classes que mostram o texto (no trilho recolhido, só o avatar aparece). */
    reveal?: string
  } = $props()

  let photoOpen = $state(false)

  // Uma carga por sessão (o menu aparece na barra lateral e no menu do celular).
  $effect(() => {
    if (session.email && !profile.loaded) loadProfile()
  })
</script>

<DropdownMenu.Root bind:open>
  <DropdownMenu.Trigger>
    {#snippet child({ props })}
      <button
        {...props}
        class="flex h-11 w-full min-w-0 cursor-pointer items-center gap-3 overflow-hidden rounded-md px-[7px] text-left whitespace-nowrap transition-colors hover:bg-sidebar-accent/60 aria-expanded:bg-sidebar-accent/60"
        aria-label="Conta"
      >
        <Avatar class="size-[26px] text-xs" />
        <span class={['flex min-w-0 flex-1 items-center gap-2 transition-opacity duration-150', reveal]}>
          <span class="min-w-0 flex-1 truncate text-sm">{session.email}</span>
          <ChevronsUpDown class="size-4 shrink-0 text-muted-foreground" />
        </span>
      </button>
    {/snippet}
  </DropdownMenu.Trigger>
  <DropdownMenu.Content side="top" align="start" class="w-(--bits-dropdown-menu-anchor-width) min-w-56">
    <div class="flex items-center gap-3 px-2 py-2">
      <Avatar class="size-9 text-sm" />
      <div class="min-w-0">
        <p class="text-xs text-muted-foreground">Conectado como</p>
        <p class="truncate text-sm font-medium">{session.email}</p>
      </div>
    </div>
    <DropdownMenu.Separator />
    <DropdownMenu.Item onclick={() => (photoOpen = true)}>
      <ImageUp />{profile.avatar === null ? 'Adicionar foto…' : 'Alterar foto…'}
    </DropdownMenu.Item>
    <DropdownMenu.Item onclick={toggleMode}>
      {#if mode.current === 'dark'}<Sun />Tema claro{:else}<Moon />Tema escuro{/if}
    </DropdownMenu.Item>
    <DropdownMenu.Item onclick={logout}><LogOut />Sair</DropdownMenu.Item>
  </DropdownMenu.Content>
</DropdownMenu.Root>

<AvatarDialog bind:open={photoOpen} />
