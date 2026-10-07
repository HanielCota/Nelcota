<script lang="ts">
  import { Button } from '$lib/components/ui/button'
  import ArrowLeft from '@lucide/svelte/icons/arrow-left'
  import Compass from '@lucide/svelte/icons/compass'
  import { href } from '$lib/router.svelte'
</script>

<div class="grid h-full place-items-center p-8 text-center">
  <div class="grid justify-items-center">
    <span class="grid size-16 place-items-center rounded-2xl border border-brand/20 bg-brand-soft text-brand shadow-card">
      <Compass class="size-7" strokeWidth={1.5} />
    </span>
    <p class="mt-6 font-mono text-sm font-medium tracking-widest text-brand">ERRO 404</p>
    <h1 class="mt-2 text-3xl font-bold tracking-tight">Página não encontrada</h1>
    <p class="mt-2 max-w-sm text-base text-muted-foreground">O endereço não existe neste painel. Confira o link ou volte ao início.</p>
    <Button class="mt-8" href={href('/')}><ArrowLeft />Voltar à visão geral</Button>
  </div>
</div>
