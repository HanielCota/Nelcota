<script lang="ts">
  import { Button } from '$lib/components/ui/button'
  import { Input } from '$lib/components/ui/input'
  import { Label } from '$lib/components/ui/label'
  import Logo from '$lib/components/app/Logo.svelte'
  import { api } from '$lib/api'
  import { session } from '$lib/session.svelte'

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

<div class="grid min-h-screen place-items-center p-4">
  <form class="grid w-full max-w-xs gap-4" onsubmit={submit}>
    <div class="mb-2">
      <Logo class="text-lg" />
      <p class="text-sm text-muted-foreground">Painel administrativo</p>
    </div>
    <div class="grid gap-1.5">
      <Label for="email">Email</Label>
      <Input id="email" type="email" autocomplete="username" bind:value={email} required />
    </div>
    <div class="grid gap-1.5">
      <Label for="password">Senha</Label>
      <Input id="password" type="password" autocomplete="current-password" bind:value={password} required />
    </div>
    {#if error}
      <p class="text-sm text-destructive">{error}</p>
    {/if}
    <Button type="submit" disabled={loading}>{loading ? 'Entrando…' : 'Entrar'}</Button>
  </form>
</div>
