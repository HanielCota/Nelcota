<script lang="ts">
  import { onMount } from 'svelte'
  import { Button } from '$lib/components/ui/button'
  import { Input } from '$lib/components/ui/input'
  import { Label } from '$lib/components/ui/label'
  import { api } from '$lib/api'
  import { session } from '$lib/session.svelte'
  import Mascot, { type Pose } from '$lib/components/app/Mascot.svelte'
  import type { Point } from '$lib/mascot'
  import { errorMessage, i18n, setLocale, t } from '$lib/i18n/index.svelte'

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

  async function submit(event: SubmitEvent) {
    event.preventDefault()
    loading = true
    failure = null
    try {
      const me = await api.post<{ email: string }>('/login', { email, password })
      session.email = me.email
    } catch (e) {
      failure = e
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

<main class="flex min-h-screen flex-col items-center bg-background px-4 pt-[max(10rem,27vh)] pb-12">
  <div class="relative w-full max-w-[400px]">
    <Mascot {pose} lookAt={caret} class="pointer-events-none absolute -top-[8.6rem] left-1/2 size-36 -translate-x-1/2" />

    <form class="grid gap-5 rounded-lg border bg-card px-6 pt-10 pb-6 sm:px-8 sm:pb-8" onsubmit={submit}>
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
        <Input
          id="password"
          type="password"
          autocomplete="current-password"
          class="h-10"
          bind:value={password}
          onfocus={() => (typingPassword = true)}
          onblur={() => (typingPassword = false)}
          oninput={() => (failure = null)}
          required
        />
      </div>

      {#if failure}
        <p class="text-sm text-destructive" role="alert">{errorMessage(failure)}</p>
      {/if}

      <Button type="submit" class="mt-1 h-10 w-full" disabled={loading}>{loading ? t('login.signingIn') : t('login.signIn')}</Button>
    </form>

    <p class="mt-6 text-center text-sm text-muted-foreground">{t('login.restricted')}</p>
    <!-- Each language names itself, so it can be found from either one. -->
    <p class="mt-3 text-center text-sm">
      {#if i18n.locale === 'pt-BR'}
        <button type="button" lang="en" class="cursor-pointer text-muted-foreground underline-offset-4 hover:text-foreground hover:underline" onclick={() => setLocale('en')}>English</button>
      {:else}
        <button type="button" lang="pt-BR" class="cursor-pointer text-muted-foreground underline-offset-4 hover:text-foreground hover:underline" onclick={() => setLocale('pt-BR')}>Português</button>
      {/if}
    </p>
  </div>
</main>
