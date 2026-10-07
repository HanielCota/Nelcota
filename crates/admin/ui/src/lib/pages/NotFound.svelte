<script lang="ts">
  import { Button } from '$lib/components/ui/button'
  import { href } from '$lib/router.svelte'
</script>

<div class="grid h-full place-items-center p-8 text-center">
  <div>
    <p class="font-mono text-sm text-muted-foreground">404</p>
    <h1 class="mt-2 text-xl font-semibold">Página não encontrada</h1>
    <p class="mt-1 text-sm text-muted-foreground">O endereço não existe neste painel.</p>
    <Button variant="outline" class="mt-6" href={href('/')}>Voltar à visão geral</Button>
  </div>
</div>
