<script lang="ts">
  import * as Select from '$lib/components/ui/select'
  import { Button } from '$lib/components/ui/button'
  import KeyRound from '@lucide/svelte/icons/key-round'
  import ShieldAlert from '@lucide/svelte/icons/shield-alert'
  import { toast } from 'svelte-sonner'
  import CodeBlock from './CodeBlock.svelte'
  import { api } from '$lib/api'

  const DURATIONS = [
    { value: '30', label: '30 dias' },
    { value: '90', label: '90 dias' },
    { value: '365', label: '1 ano' },
  ]

  let days = $state('90')
  let issued = $state<{ token: string; expires_at: number } | null>(null)
  let busy = $state(false)

  async function issue() {
    busy = true
    try {
      issued = await api.post<{ token: string; expires_at: number }>('/tokens/service-role', { days: Number(days) })
    } catch (e) {
      toast.error((e as Error).message)
    } finally {
      busy = false
    }
  }

  const expires = (epoch: number) =>
    new Intl.DateTimeFormat('pt-BR', { dateStyle: 'long' }).format(new Date(epoch * 1000))
</script>

<!-- O token não é guardado: aparece uma vez, aqui, para copiar. -->
<section class="grid gap-5 rounded-xl border border-warning/30 bg-card p-5 shadow-card sm:p-6">
  <div class="flex gap-4">
    <span class="grid size-10 shrink-0 place-items-center rounded-xl border border-warning/25 bg-warning/10 text-warning">
      <KeyRound class="size-5" />
    </span>
    <div>
      <h2 class="text-base font-semibold">Token <code class="font-mono">service_role</code></h2>
      <p class="mt-1 text-sm leading-relaxed text-muted-foreground">
        Para o seu backend: <strong class="font-semibold text-foreground">ignora todo o RLS</strong>. Nunca coloque no
        frontend, num app mobile ou num repositório. Cada token vale até expirar. Ainda depende de GRANT em cada
        tabela (as criadas pelo painel já dão acesso total ao service_role).
      </p>
    </div>
  </div>

  {#if issued}
    <div class="grid gap-2">
      <CodeBlock code={issued.token} label="Copiar token" wrap />
      <p class="flex items-center gap-2 text-sm font-medium text-warning">
        <ShieldAlert class="size-4 shrink-0" />Copie agora: ele não será mostrado de novo. Vale até {expires(issued.expires_at)}.
      </p>
    </div>
  {/if}

  <div class="flex flex-wrap items-center gap-2">
    <Select.Root type="single" bind:value={days}>
      <Select.Trigger class="w-36" aria-label="Validade">
        {DURATIONS.find((d) => d.value === days)?.label}
      </Select.Trigger>
      <Select.Content>
        {#each DURATIONS as duration (duration.value)}
          <Select.Item value={duration.value}>{duration.label}</Select.Item>
        {/each}
      </Select.Content>
    </Select.Root>
    <Button variant="outline" disabled={busy} onclick={issue}>
      {busy ? 'Gerando…' : issued ? 'Gerar outro' : 'Gerar token'}
    </Button>
    <span class="text-sm text-muted-foreground">
      ou no servidor: <code class="text-foreground">nelcota token service-role</code>
    </span>
  </div>
</section>
