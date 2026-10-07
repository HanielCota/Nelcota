<script lang="ts">
  import { Button } from '$lib/components/ui/button'
  import { href } from '$lib/router.svelte'
  import { t } from '$lib/i18n/index.svelte'
</script>

<div class="grid h-full place-items-center p-8 text-center">
  <div>
    <p class="font-mono text-sm text-muted-foreground">404</p>
    <h1 class="mt-2 text-xl font-semibold">{t('overview.notFound.title')}</h1>
    <p class="mt-1 text-sm text-muted-foreground">{t('overview.notFound.description')}</p>
    <Button variant="outline" class="mt-6" href={href('/')}>{t('overview.notFound.back')}</Button>
  </div>
</div>
