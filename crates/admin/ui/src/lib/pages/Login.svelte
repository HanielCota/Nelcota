<script lang="ts">
  import { onMount } from 'svelte'
  import { Button } from '$lib/components/ui/button'
  import { Input } from '$lib/components/ui/input'
  import { Label } from '$lib/components/ui/label'
  import Logo from '$lib/components/app/Logo.svelte'
  import { api } from '$lib/api'
  import { session } from '$lib/session.svelte'

  let project = $state('')
  onMount(async () => {
    try {
      project = (await api.get<{ project: string }>('/whoami')).project
    } catch {
      project = ''
    }
  })

  let email = $state('')
  let password = $state('')
  let error = $state('')
  let loading = $state(false)

  async function submit(event: SubmitEvent) {
    event.preventDefault()
    loading = true
    error = ''
    try {
      const me = await api.post<{ email: string }>('/login', { email, password })
      session.email = me.email
    } catch (e) {
      error = (e as Error).message
    } finally {
      loading = false
    }
  }
</script>

<main class="flex min-h-screen flex-col items-center justify-center bg-background px-6 py-12">
  <form class="grid w-full max-w-sm gap-5" onsubmit={submit}>
    <Logo class="mb-4" />
    <div class="grid gap-1">
      <h1 class="text-xl font-semibold">Entrar no painel</h1>
      {#if project}
        <p class="text-sm text-muted-foreground">Projeto <span class="font-medium text-foreground">{project}</span></p>
      {/if}
    </div>
    <div class="grid gap-2">
      <Label for="email">Email</Label>
      <Input id="email" type="email" autocomplete="username" class="h-10" bind:value={email} required />
    </div>
    <div class="grid gap-2">
      <Label for="password">Senha</Label>
      <Input
        id="password"
        type="password"
        autocomplete="current-password"
        class="h-10"
        bind:value={password}
        required
      />
    </div>
    {#if error}
      <p class="text-sm text-destructive" role="alert">{error}</p>
    {/if}
    <Button type="submit" class="mt-1 h-10" disabled={loading}>{loading ? 'Entrando…' : 'Entrar'}</Button>
    <p class="mt-4 text-xs text-muted-foreground">Acesso restrito aos administradores deste host.</p>
  </form>
</main>
