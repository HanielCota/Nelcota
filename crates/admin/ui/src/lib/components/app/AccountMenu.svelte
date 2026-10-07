<script lang="ts">
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu'
  import { mode, toggleMode } from 'mode-watcher'
  import Sun from '@lucide/svelte/icons/sun'
  import Moon from '@lucide/svelte/icons/moon'
  import LogOut from '@lucide/svelte/icons/log-out'
  import ChevronsUpDown from '@lucide/svelte/icons/chevrons-up-down'
  import ImageUp from '@lucide/svelte/icons/image-up'
  import Languages from '@lucide/svelte/icons/languages'
  import Avatar from './Avatar.svelte'
  import AvatarDialog from './AvatarDialog.svelte'
  import { logout } from '$lib/auth'
  import { loadProfile, profile } from '$lib/profile.svelte'
  import { session } from '$lib/session.svelte'
  import { LOCALES, i18n, setLocale, t, type Locale } from '$lib/i18n/index.svelte'

  let {
    open = $bindable(false),
    reveal = '',
  }: {
    open?: boolean
    /** Classes that reveal the text (in the collapsed rail only the avatar shows). */
    reveal?: string
  } = $props()

  let photoOpen = $state(false)

  // One load per session (the menu shows in the sidebar and in the phone menu).
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
        aria-label={t('shell.account.label')}
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
        <p class="text-xs text-muted-foreground">{t('shell.account.signedInAs')}</p>
        <p class="truncate text-sm font-medium">{session.email}</p>
      </div>
    </div>
    <DropdownMenu.Separator />
    <DropdownMenu.Item onclick={() => (photoOpen = true)}>
      <ImageUp />{profile.avatar === null ? t('shell.account.addPhoto') : t('shell.account.changePhoto')}
    </DropdownMenu.Item>
    <DropdownMenu.Item onclick={toggleMode}>
      {#if mode.current === 'dark'}<Sun />{t('shell.account.lightTheme')}{:else}<Moon />{t('shell.account.darkTheme')}{/if}
    </DropdownMenu.Item>
    <DropdownMenu.Sub>
      <DropdownMenu.SubTrigger>
        <Languages />{t('shell.account.language')}
        <span class="ml-auto pl-3 text-xs text-muted-foreground">{t(`shell.languages.${i18n.locale}`)}</span>
      </DropdownMenu.SubTrigger>
      <DropdownMenu.SubContent class="min-w-40">
        <DropdownMenu.RadioGroup value={i18n.locale} onValueChange={(value) => setLocale(value as Locale)}>
          {#each LOCALES as locale (locale)}
            <!-- Each language names itself, so it can be found from either one. -->
            <DropdownMenu.RadioItem value={locale} lang={locale}>{t(`shell.languages.${locale}`)}</DropdownMenu.RadioItem>
          {/each}
        </DropdownMenu.RadioGroup>
      </DropdownMenu.SubContent>
    </DropdownMenu.Sub>
    <DropdownMenu.Separator />
    <DropdownMenu.Item onclick={logout}><LogOut />{t('shell.account.signOut')}</DropdownMenu.Item>
  </DropdownMenu.Content>
</DropdownMenu.Root>

<AvatarDialog bind:open={photoOpen} />
