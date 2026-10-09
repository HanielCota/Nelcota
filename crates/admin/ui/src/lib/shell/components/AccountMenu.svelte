<script lang="ts">
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu'
  import { mode, toggleMode } from 'mode-watcher'
  import Sun from '@lucide/svelte/icons/sun'
  import Moon from '@lucide/svelte/icons/moon'
  import LogOut from '@lucide/svelte/icons/log-out'
  import ChevronsUpDown from '@lucide/svelte/icons/chevrons-up-down'
  import ImageUp from '@lucide/svelte/icons/image-up'
  import Languages from '@lucide/svelte/icons/languages'
  import Avatar from '$lib/features/profile/components/Avatar.svelte'
  import AvatarDialog from '$lib/features/profile/components/AvatarDialog.svelte'
  import { logout } from '$lib/features/auth/auth'
  import { loadProfile, profile } from '$lib/features/profile/profile.svelte'
  import { session } from '$lib/features/auth/session.svelte'
  import { LOCALES, i18n, setLocale, t, type Locale } from '$lib/i18n/index.svelte'

  let { open = $bindable(false) }: { open?: boolean } = $props()

  let photoOpen = $state(false)

  // One load per session.
  $effect(() => {
    if (session.email && !profile.loaded) loadProfile()
  })
</script>

<DropdownMenu.Root bind:open>
  <DropdownMenu.Trigger>
    {#snippet child({ props })}
      <button
        {...props}
        class="flex h-12 max-w-64 min-w-0 cursor-pointer items-center gap-2.5 overflow-hidden rounded-full bg-nav py-1.5 pr-3.5 pl-1.5 text-left whitespace-nowrap transition-colors hover:bg-accent aria-expanded:bg-accent"
        aria-label={t('shell.account.label')}
      >
        <Avatar class="size-9 text-sm" />
        <span class="hidden min-w-0 flex-1 items-center gap-2 sm:flex">
          <span class="min-w-0 flex-1 truncate text-sm font-medium">{session.email}</span>
          <ChevronsUpDown class="size-4 shrink-0 text-muted-foreground" />
        </span>
      </button>
    {/snippet}
  </DropdownMenu.Trigger>
  <DropdownMenu.Content side="bottom" align="end" class="min-w-64">
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
