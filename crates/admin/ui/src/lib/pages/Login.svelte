<script lang="ts">
  import { Button } from '$lib/components/ui/button'
  import { Input } from '$lib/components/ui/input'
  import { Label } from '$lib/components/ui/label'
  import * as Card from '$lib/components/ui/card'
  import LoaderCircle from '@lucide/svelte/icons/loader-circle'
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

<div class="login-bg grid min-h-screen place-items-center p-4">
  <div class="w-full max-w-sm">
    <div class="mb-6 flex items-center justify-center gap-2.5">
      <Logo class="size-9" />
      <span class="text-lg font-semibold tracking-tight">nelcota</span>
    </div>
    <Card.Root>
      <Card.Header>
        <Card.Title class="text-lg">Bem-vindo de volta</Card.Title>
        <Card.Description>Entre no painel administrativo</Card.Description>
      </Card.Header>
      <Card.Content>
        <form class="grid gap-4" onsubmit={submit}>
          <div class="grid gap-2">
            <Label for="email">Email</Label>
            <Input
              id="email"
              type="email"
              autocomplete="username"
              placeholder="admin@seudominio.com"
              bind:value={email}
              required
            />
          </div>
          <div class="grid gap-2">
            <Label for="password">Senha</Label>
            <Input
              id="password"
              type="password"
              autocomplete="current-password"
              placeholder="••••••••"
              bind:value={password}
              required
            />
          </div>
          {#if error}
            <p class="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-sm text-destructive">
              {error}
            </p>
          {/if}
          <Button type="submit" class="w-full" disabled={loading}>
            {#if loading}<LoaderCircle class="animate-spin" />{/if}
            Entrar
          </Button>
        </form>
      </Card.Content>
    </Card.Root>
    <p class="mt-4 text-center text-xs text-muted-foreground">
      Login do administrador, separado dos usuários finais da sua aplicação.
    </p>
  </div>
</div>

<style>
  .login-bg {
    background: radial-gradient(
      ellipse at top,
      color-mix(in oklch, var(--primary) 14%, transparent),
      transparent 60%
    );
  }
</style>
