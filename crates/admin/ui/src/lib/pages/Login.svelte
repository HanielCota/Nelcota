<script lang="ts">
  import { onMount } from 'svelte'
  import { Button } from '$lib/components/ui/button'
  import { Input } from '$lib/components/ui/input'
  import { Label } from '$lib/components/ui/label'
  import Mail from '@lucide/svelte/icons/mail'
  import LockKeyhole from '@lucide/svelte/icons/lock-keyhole'
  import ArrowRight from '@lucide/svelte/icons/arrow-right'
  import ShieldCheck from '@lucide/svelte/icons/shield-check'
  import Database from '@lucide/svelte/icons/database'
  import KeyRound from '@lucide/svelte/icons/key-round'
  import Logo from '$lib/components/app/Logo.svelte'
  import Callout from '$lib/components/app/Callout.svelte'
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

  const features = [
    { icon: Database, text: 'Postgres com API REST gerada a partir do schema' },
    { icon: KeyRound, text: 'Autenticação com JWT e sessões renováveis' },
    { icon: ShieldCheck, text: 'Row Level Security editável pelo painel' },
  ]
</script>

<main class="grid min-h-screen bg-background lg:grid-cols-[minmax(0,1fr)_minmax(0,1.1fr)]">
  <div class="flex flex-col px-6 py-8 sm:px-12">
    <Logo />
    <div class="flex flex-1 items-center justify-center py-12">
      <form class="grid w-full max-w-sm gap-6" onsubmit={submit}>
        <div class="mb-2 grid gap-2">
          <h1 class="text-3xl font-bold tracking-tight sm:text-4xl">Bem-vindo de volta</h1>
          <p class="text-base text-muted-foreground">
            Entre no painel {#if project}do projeto <span class="font-semibold text-foreground">{project}</span
              >{:else}administrativo{/if}.
          </p>
        </div>
        <div class="grid gap-2">
          <Label for="email">Email</Label>
          <div class="relative">
            <Mail class="pointer-events-none absolute top-1/2 left-3.5 size-4 -translate-y-1/2 text-muted-foreground" />
            <Input
              id="email"
              type="email"
              autocomplete="username"
              placeholder="voce@exemplo.com"
              class="h-11 pl-10 text-base md:text-base"
              bind:value={email}
              required
            />
          </div>
        </div>
        <div class="grid gap-2">
          <Label for="password">Senha</Label>
          <div class="relative">
            <LockKeyhole
              class="pointer-events-none absolute top-1/2 left-3.5 size-4 -translate-y-1/2 text-muted-foreground"
            />
            <Input
              id="password"
              type="password"
              autocomplete="current-password"
              placeholder="••••••••"
              class="h-11 pl-10 text-base md:text-base"
              bind:value={password}
              required
            />
          </div>
        </div>
        {#if error}
          <Callout variant="danger">{error}</Callout>
        {/if}
        <Button type="submit" size="lg" class="mt-1 w-full" disabled={loading}>
          {#if loading}
            <span class="size-4 animate-spin rounded-full border-2 border-current/30 border-t-current"></span>Entrando…
          {:else}
            Entrar<ArrowRight />
          {/if}
        </Button>
      </form>
    </div>
    <p class="text-center text-sm text-muted-foreground lg:text-left">Acesso restrito aos administradores deste host.</p>
  </div>

  <aside
    class="relative hidden overflow-hidden border-l border-sidebar-border bg-sidebar lg:flex lg:items-center lg:justify-center"
  >
    <div
      class="absolute inset-0 [background-image:linear-gradient(var(--border)_1px,transparent_1px),linear-gradient(90deg,var(--border)_1px,transparent_1px)] [background-size:44px_44px] [mask-image:radial-gradient(ellipse_at_center,black_20%,transparent_75%)] opacity-60"
    ></div>
    <div class="absolute top-1/4 left-1/2 size-[28rem] -translate-x-1/2 rounded-full bg-brand/15 blur-3xl"></div>
    <div class="relative max-w-lg px-12">
      <span
        class="inline-flex items-center gap-2 rounded-full border border-brand/30 bg-brand-soft px-3 py-1 text-xs font-semibold text-brand"
      >
        <span class="size-1.5 rounded-full bg-brand"></span>Backend completo num binário só
      </span>
      <p class="mt-5 text-3xl leading-tight font-bold tracking-tight text-foreground">
        Postgres com <span class="text-brand">API REST</span>, autenticação e políticas de RLS.
      </p>
      <ul class="mt-8 grid gap-3">
        {#each features as feature (feature.text)}
          <li class="flex items-center gap-3 text-sm text-muted-foreground">
            <span class="grid size-8 shrink-0 place-items-center rounded-lg border bg-card text-brand shadow-card">
              <feature.icon class="size-4" strokeWidth={1.75} />
            </span>
            {feature.text}
          </li>
        {/each}
      </ul>
      <div class="mt-10 overflow-hidden rounded-xl border bg-card/80 font-mono text-xs leading-relaxed shadow-overlay backdrop-blur">
        <div class="flex items-center gap-1.5 border-b bg-muted/50 px-4 py-2.5">
          <span class="size-2.5 rounded-full bg-border-strong"></span>
          <span class="size-2.5 rounded-full bg-border-strong"></span>
          <span class="size-2.5 rounded-full bg-border-strong"></span>
        </div>
        <div class="p-4">
          <p><span class="text-muted-foreground">$</span> curl {'/rest/v1/notas?select=*'}</p>
          <p class="mt-2 text-muted-foreground">{'['}</p>
          <p class="pl-4"><span class="text-brand">"id"</span>: 1, <span class="text-brand">"texto"</span>: "olá"</p>
          <p class="text-muted-foreground">{']'}</p>
        </div>
      </div>
    </div>
  </aside>
</main>
