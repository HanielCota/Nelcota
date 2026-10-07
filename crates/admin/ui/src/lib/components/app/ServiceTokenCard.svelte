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
<section class="grid gap-4 rounded-lg border border-warning/30 bg-card p-5">
  <div class="flex gap-3">
    <KeyRound class="mt-0.5 size-5 shrink-0 text-warning" />
    <div>
      <h2 class="text-sm font-medium">Token service_role</h2>
      <p class="mt-1 text-sm font-light text-muted-foreground">
        Para o seu backend: <strong class="font-medium text-foreground">ignora todo o RLS</strong>. Nunca coloque no
        frontend, num app mobile ou num repositório. Cada token vale até expirar. Ainda depende de GRANT em cada
        tabela (as criadas pelo painel já dão acesso total ao service_role).
      </p>
    </div>
  </div>

  {#if issued}
    <div class="grid gap-2">
      <CodeBlock code={issued.token} label="Copiar token" wrap />
      <p class="flex items-center gap-1.5 text-xs text-warning">
        <ShieldAlert class="size-3.5" />Copie agora: ele não será mostrado de novo. Vale até {expires(issued.expires_at)}.
      </p>
    </div>
  {/if}

  <div class="flex flex-wrap items-center gap-2">
    <Select.Root type="single" bind:value={days}>
      <Select.Trigger size="sm" class="w-32" aria-label="Validade">
        {DURATIONS.find((d) => d.value === days)?.label}
      </Select.Trigger>
      <Select.Content>
        {#each DURATIONS as duration (duration.value)}
          <Select.Item value={duration.value}>{duration.label}</Select.Item>
        {/each}
      </Select.Content>
    </Select.Root>
    <Button size="sm" variant="outline" disabled={busy} onclick={issue}>
      {busy ? 'Gerando…' : issued ? 'Gerar outro' : 'Gerar token'}
    </Button>
    <span class="text-xs font-light text-muted-foreground">
      ou no servidor: <code class="text-foreground">nelcota token service-role</code>
    </span>
  </div>
</section>
