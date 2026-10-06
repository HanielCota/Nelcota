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

<div class="grid min-h-screen bg-background lg:grid-cols-[minmax(0,1fr)_minmax(0,1.1fr)]">
  <div class="flex flex-col px-6 py-8 sm:px-10">
    <Logo />
    <div class="flex flex-1 items-center justify-center py-12">
      <form class="grid w-full max-w-sm gap-5" onsubmit={submit}>
        <div class="mb-3 grid gap-1.5">
          <h1 class="text-3xl font-medium tracking-tight">Bem-vindo de volta</h1>
          <p class="text-sm font-light text-muted-foreground">
            Entre no painel {#if project}do projeto <span class="font-medium text-foreground">{project}</span
              >{:else}administrativo{/if}
          </p>
        </div>
        <div class="grid gap-2">
          <Label for="email" class="text-sm font-normal text-muted-foreground">Email</Label>
          <Input
            id="email"
            type="email"
            autocomplete="username"
            placeholder="voce@exemplo.com"
            class="h-10 bg-card"
            bind:value={email}
            required
          />
        </div>
        <div class="grid gap-2">
          <Label for="password" class="text-sm font-normal text-muted-foreground">Senha</Label>
          <Input
            id="password"
            type="password"
            autocomplete="current-password"
            placeholder="••••••••"
            class="h-10 bg-card"
            bind:value={password}
            required
          />
        </div>
        {#if error}
          <p class="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-sm text-destructive">
            {error}
          </p>
        {/if}
        <Button type="submit" class="mt-1 h-10" disabled={loading}>{loading ? 'Entrando…' : 'Entrar'}</Button>
      </form>
    </div>
    <p class="text-center text-xs font-light text-muted-foreground lg:text-left">
      Acesso restrito aos administradores deste host.
    </p>
  </div>

  <aside
    class="relative hidden overflow-hidden border-l border-sidebar-border bg-sidebar lg:flex lg:items-center lg:justify-center"
  >
    <div
      class="absolute inset-0 [background-image:linear-gradient(var(--border)_1px,transparent_1px),linear-gradient(90deg,var(--border)_1px,transparent_1px)] [background-size:44px_44px] [mask-image:radial-gradient(ellipse_at_center,black_20%,transparent_75%)] opacity-60"
    ></div>
    <div class="absolute top-1/3 left-1/2 size-96 -translate-x-1/2 rounded-full bg-brand/10 blur-3xl"></div>
    <div class="relative max-w-md px-10">
      <p class="text-2xl leading-snug font-light tracking-tight text-foreground">
        Postgres com <span class="font-medium text-brand">API REST</span>, autenticação e políticas de RLS —
        num binário só.
      </p>
      <div class="mt-8 rounded-lg border bg-card/80 p-4 font-mono text-xs leading-relaxed shadow-2xl backdrop-blur">
        <p><span class="text-muted-foreground">$</span> curl {'/rest/v1/notas?select=*'}</p>
        <p class="mt-2 text-muted-foreground">{'['}</p>
        <p class="pl-4"><span class="text-brand">"id"</span>: 1, <span class="text-brand">"texto"</span>: "olá"</p>
        <p class="text-muted-foreground">{']'}</p>
      </div>
    </div>
  </aside>
</div>
