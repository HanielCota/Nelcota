<script lang="ts">
  import { onMount } from 'svelte'
  import { ModeWatcher } from 'mode-watcher'
  import Login from '$lib/features/auth/Login.svelte'
  import LoadingScreen from '$lib/shell/LoadingScreen.svelte'
  import { takeHandoffToken } from '$lib/features/projects/projects'
  import { api } from '$lib/api'
  import { session } from '$lib/features/auth/session.svelte'
  import { route } from '$lib/router.svelte'
  import { crumbsFor, documentTitle } from '$lib/shell/titles'
  import { errorMessage, t } from '$lib/i18n/index.svelte'

  onMount(async () => {
    // Coming from another panel on the host (single sign-on): trade the token for a session.
    const token = takeHandoffToken()
    if (token) {
      try {
        session.email = (await api.post<{ email: string }>('/sso', { token })).email
        return
      } catch {
        // Expired or already used token: fall back to the normal login.
      }
    }
    try {
      const me = await api.get<{ email: string }>('/session')
      session.email = me.email
    } catch {
      session.email = null
    }
  })

  // Tab title: the current page (or the login) followed by the product name.
  const title = $derived(
    session.email === null ? `${t('shell.pages.signIn')} · Nelcota` : session.email ? documentTitle(crumbsFor(route.path)) : 'Nelcota',
  )
</script>

<svelte:head><title>{title}</title></svelte:head>

<ModeWatcher defaultMode="dark" />
{#if session.email === undefined}
  <LoadingScreen />
{:else if session.email === null}
  <Login />
{:else}
  {#await import('$lib/shell/AuthenticatedShell.svelte')}
    <LoadingScreen />
  {:then shell}
    <shell.default />
  {:catch error}
    <div class="p-6" role="alert">
      <p>{errorMessage(error)}</p>
      <button class="mt-3 underline" onclick={() => window.location.reload()}>{t('common.retry')}</button>
    </div>
  {/await}
{/if}
