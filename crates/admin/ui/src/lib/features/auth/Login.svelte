<script lang="ts">
  import { onMount } from 'svelte'
  import { Button } from '$lib/components/ui/button'
  import { Input } from '$lib/components/ui/input'
  import { Label } from '$lib/components/ui/label'
  import Eye from '@lucide/svelte/icons/eye'
  import EyeOff from '@lucide/svelte/icons/eye-off'
  import LoaderCircle from '@lucide/svelte/icons/loader-circle'
  import Sun from '@lucide/svelte/icons/sun'
  import Moon from '@lucide/svelte/icons/moon'
  import { mode, toggleMode } from 'mode-watcher'
  import { api } from '$lib/api'
  import { session } from '$lib/features/auth/session.svelte'
  import Mascot, { type Pose } from '$lib/shell/components/Mascot.svelte'
  import type { Point } from '$lib/shell/mascot'
  import { LOCALES, errorMessage, i18n, setLocale, t, type Locale } from '$lib/i18n/index.svelte'

  // Short codes on the switch; each option names its language in full for
  // screen readers and on hover, in that language.
  const CODES: Record<Locale, string> = { 'pt-BR': 'PT', en: 'EN' }

  let project = $state('')
  // The mascot waves when the page opens, then stands still.
  let greeting = $state(true)
  onMount(() => {
    const timer = setTimeout(() => (greeting = false), 2200)
    api
      .get<{ project: string }>('/whoami')
      .then((r) => (project = r.project))
      .catch(() => (project = ''))
    return () => clearTimeout(timer)
  })

  let email = $state('')
  let password = $state('')
  // The failure itself (not its text), so the message follows a language switch.
  let failure = $state<unknown>(null)
  let loading = $state(false)
  let typingPassword = $state(false)
  let showPassword = $state(false)

  async function submit(event: SubmitEvent) {
    event.preventDefault()
    if (loading) return
    loading = true
    failure = null
    try {
      const me = await api.post<{ email: string }>('/login', { email, password })
      session.email = me.email
    } catch (e) {
      failure = e
      document.getElementById('password')?.focus()
    } finally {
      loading = false
    }
  }

  // Closes its eyes while the password is typed; looks sad after a failed login.
  const pose = $derived<Pose>(typingPassword ? 'eyesClosed' : failure ? 'sad' : greeting ? 'wave' : 'neutral')

  // While the email is typed, follows the text instead of the pointer.
  let caret = $state<Point | null>(null)
  function followCaret(event: Event) {
    const input = event.currentTarget as HTMLInputElement
    const rect = input.getBoundingClientRect()
    const chars = input.selectionStart ?? input.value.length
    // Average character width at 15px: close enough for the gaze.
    caret = { x: rect.left + Math.min(12 + chars * 8, rect.width - 12), y: rect.top + rect.height / 2 }
  }
</script>

<main class="relative flex min-h-screen flex-col items-center bg-background px-4 pt-[max(10rem,27vh)] pb-12">
  <!-- Language and theme, in the corner (the account menu is not there yet). -->
  <div class="absolute top-4 right-4 flex items-center gap-2 sm:top-6 sm:right-6">
    <div class="flex items-center gap-1 rounded-full bg-card p-1" role="group" aria-label={t('shell.account.language')}>
      {#each LOCALES as locale (locale)}
        {@const current = i18n.locale === locale}
        <button
          type="button"
          lang={locale}
          aria-pressed={current}
          aria-label={t(`shell.languages.${locale}`)}
          title={t(`shell.languages.${locale}`)}
          class={[
            'h-8 cursor-pointer rounded-full px-3 text-xs font-medium transition-colors',
            current ? 'bg-nav-active text-nav-active-foreground' : 'text-muted-foreground hover:text-foreground',
          ]}
          onclick={() => setLocale(locale)}>{CODES[locale]}</button
        >
      {/each}
    </div>
    <button
      type="button"
      class="grid size-10 cursor-pointer place-items-center rounded-full bg-card text-muted-foreground transition-colors hover:text-foreground"
      onclick={toggleMode}
      aria-label={mode.current === 'dark' ? t('shell.account.lightTheme') : t('shell.account.darkTheme')}
      title={mode.current === 'dark' ? t('shell.account.lightTheme') : t('shell.account.darkTheme')}
    >
      {#if mode.current === 'dark'}<Sun class="size-[18px]" aria-hidden="true" />{:else}<Moon class="size-[18px]" aria-hidden="true" />{/if}
    </button>
  </div>

  <div class="relative w-full max-w-[400px]">
    <Mascot {pose} lookAt={caret} class="pointer-events-none absolute -top-[8.6rem] left-1/2 size-36 -translate-x-1/2" />

    <form class="grid gap-5 rounded-3xl bg-card px-6 pt-10 pb-6 sm:px-8 sm:pb-8" onsubmit={submit}>
      <div class="text-center">
        <h1 class="text-xl font-semibold tracking-tight">{t('login.title')}</h1>
        <p class="mt-1 text-sm text-muted-foreground">
          {#if project}{t('login.projectPanel')} <span class="font-medium text-foreground">{project}</span>{:else}{t('login.adminPanel')}{/if}
        </p>
      </div>

      <div class="grid gap-2">
        <Label for="email">{t('login.email')}</Label>
        <Input
          id="email"
          type="email"
          autocomplete="username"
          class="h-10"
          bind:value={email}
          onfocus={followCaret}
          onkeyup={followCaret}
          onclick={followCaret}
          onblur={() => (caret = null)}
          oninput={(e) => {
            failure = null
            followCaret(e)
          }}
          required
        />
      </div>
      <div class="grid gap-2">
        <Label for="password">{t('login.password')}</Label>
        <div class="relative">
        <Input
          id="password"
          type={showPassword ? 'text' : 'password'}
          autocomplete="current-password"
          class="h-10 pr-11"
          bind:value={password}
          onfocus={() => (typingPassword = true)}
          onblur={() => (typingPassword = false)}
          oninput={() => (failure = null)}
          required
          aria-invalid={failure ? true : undefined}
          aria-describedby={failure ? 'login-error' : undefined}
        />
        <Button type="button" variant="ghost" size="icon-sm" class="absolute top-1 right-1 z-10" aria-label={t(showPassword ? 'common.hidePassword' : 'common.showPassword')} aria-pressed={showPassword} onclick={() => { showPassword = !showPassword; document.getElementById('password')?.focus() }}>{#if showPassword}<EyeOff aria-hidden="true" />{:else}<Eye aria-hidden="true" />{/if}</Button>
        </div>
      </div>

      {#if failure}
        <p id="login-error" class="text-sm text-destructive" role="alert">{errorMessage(failure)}</p>
      {/if}

      <Button type="submit" class="mt-1 h-10 w-full" disabled={loading}>{#if loading}<LoaderCircle data-icon="inline-start" class="animate-spin" aria-hidden="true" />{/if}{loading ? t('login.signingIn') : t('login.signIn')}</Button>
    </form>

    <p class="mt-6 text-center text-sm text-muted-foreground">{t('login.restricted')}</p>
  </div>
</main>
