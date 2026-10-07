<script lang="ts">
  import { onMount } from 'svelte'
  import { Button } from '$lib/components/ui/button'
  import { Input } from '$lib/components/ui/input'
  import { Label } from '$lib/components/ui/label'
  import { api } from '$lib/api'
  import { session } from '$lib/session.svelte'
  import neutral from '../../assets/mascot/neutral.png'
  import wave from '../../assets/mascot/wave.png'
  import eyesClosed from '../../assets/mascot/eyes-closed.png'
  import sad from '../../assets/mascot/sad.png'

  let project = $state('')
  // O mascote acena ao abrir a tela e depois fica parado.
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
  let error = $state('')
  let loading = $state(false)
  let typingPassword = $state(false)

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

  // Fecha os olhos enquanto a senha é digitada; fica triste se o login falha.
  const frames = { neutral, wave, eyesClosed, sad }
  const pose = $derived<keyof typeof frames>(
    typingPassword ? 'eyesClosed' : error ? 'sad' : greeting ? 'wave' : 'neutral',
  )
</script>

<main class="flex min-h-screen flex-col items-center justify-center bg-background px-4 pt-40 pb-12">
  <div class="relative w-full max-w-[400px]">
    <!-- Todas as poses ficam carregadas: trocar de expressão não pisca. -->
    <div class="pointer-events-none absolute -top-[8.6rem] left-1/2 size-36 -translate-x-1/2 select-none" aria-hidden="true">
      {#each Object.entries(frames) as [name, src] (name)}
        <img {src} alt="" draggable="false" class={['absolute inset-0 size-full', pose !== name && 'invisible']} />
      {/each}
    </div>

    <form class="grid gap-5 rounded-lg border bg-card px-6 pt-10 pb-6 sm:px-8 sm:pb-8" onsubmit={submit}>
      <div class="text-center">
        <h1 class="text-xl font-semibold tracking-tight">Entrar no nelcota</h1>
        <p class="mt-1 text-sm text-muted-foreground">
          {#if project}Painel do projeto <span class="font-medium text-foreground">{project}</span>{:else}Painel administrativo{/if}
        </p>
      </div>

      <div class="grid gap-2">
        <Label for="email">Email</Label>
        <Input
          id="email"
          type="email"
          autocomplete="username"
          class="h-10"
          bind:value={email}
          oninput={() => (error = '')}
          required
        />
      </div>
      <div class="grid gap-2">
        <Label for="password">Senha</Label>
        <Input
          id="password"
          type="password"
          autocomplete="current-password"
          class="h-10"
          bind:value={password}
          onfocus={() => (typingPassword = true)}
          onblur={() => (typingPassword = false)}
          oninput={() => (error = '')}
          required
        />
      </div>

      {#if error}
        <p class="text-sm text-destructive" role="alert">{error}</p>
      {/if}

      <Button type="submit" class="mt-1 h-10 w-full" disabled={loading}>{loading ? 'Entrando…' : 'Entrar'}</Button>
    </form>

    <p class="mt-6 text-center text-sm text-muted-foreground">Acesso restrito aos administradores deste host.</p>
  </div>
</main>
