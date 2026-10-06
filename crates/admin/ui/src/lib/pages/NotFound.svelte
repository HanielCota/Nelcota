<script lang="ts">
  import { Button } from '$lib/components/ui/button'
  import { href } from '$lib/router.svelte'
</script>

<div class="grid h-full place-items-center p-8 text-center">
  <div>
    <p class="text-sm text-muted-foreground">Página não encontrada.</p>
    <Button variant="outline" class="mt-4" href={href('/')}>Voltar à visão geral</Button>
  </div>
</div>
