<script lang="ts">
  import { Input } from '$lib/components/ui/input'
  import { Button } from '$lib/components/ui/button'
  import Eye from '@lucide/svelte/icons/eye'
  import EyeOff from '@lucide/svelte/icons/eye-off'
  import Copy from '@lucide/svelte/icons/copy'
  import { toast } from 'svelte-sonner'
  import { generatePassword, passwordProblem } from '$lib/password'

  let { value = $bindable(''), id }: { value?: string; id: string } = $props()

  let visible = $state(false)
  const problem = $derived(value ? passwordProblem(value) : null)

  function generate() {
    value = generatePassword()
    // Senha gerada fica visível: o admin precisa copiar para entregar ao usuário.
    visible = true
  }

  async function copy() {
    await navigator.clipboard.writeText(value)
    toast.success('Senha copiada')
  }
</script>

<div class="grid gap-1.5">
  <div class="flex gap-2">
    <div class="relative flex-1">
      <Input
        {id}
        type={visible ? 'text' : 'password'}
        bind:value
        autocomplete="new-password"
        class="h-10 pr-10 font-mono"
        aria-invalid={problem ? true : undefined}
        aria-describedby={`${id}-hint`}
      />
      <button
        type="button"
        class="absolute top-1/2 right-1.5 grid size-7 -translate-y-1/2 cursor-pointer place-items-center rounded-md text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
        aria-label={visible ? 'Ocultar senha' : 'Mostrar senha'}
        aria-pressed={visible}
        onclick={() => (visible = !visible)}
      >
        {#if visible}<EyeOff class="size-4" />{:else}<Eye class="size-4" />{/if}
      </button>
    </div>
    <Button variant="outline" class="h-10" onclick={generate}>Gerar</Button>
    <Button variant="outline" size="icon" class="size-10" aria-label="Copiar senha" disabled={!value} onclick={copy}><Copy /></Button>
  </div>
  <p id={`${id}-hint`} class={['text-xs', problem ? 'text-destructive' : 'text-muted-foreground']}>
    {problem ?? 'Mínimo de 8 caracteres. Entregue a senha ao usuário por um canal seguro.'}
  </p>
</div>
