<script lang="ts">
  import { Button } from '$lib/components/ui/button'
  import Mascot from '$lib/shell/components/Mascot.svelte'
  import { href } from '$lib/router.svelte'
  import { t } from '$lib/i18n/index.svelte'
</script>

<!-- The mascot looks lost with the visitor, on a card like the rest of the panel. -->
<div class="mx-auto grid h-full w-full max-w-page place-items-center px-4 pt-2 pb-12 sm:px-6 lg:px-8">
  <div class="grid w-full max-w-md justify-items-center rounded-xl border bg-card px-8 pt-8 pb-10 text-center">
    <Mascot pose="neutral" class="size-28" />
    <p class="mt-2 font-mono text-sm text-muted-foreground">404</p>
    <h1 class="mt-1 text-2xl font-semibold tracking-tight">{t('overview.notFound.title')}</h1>
    <p class="mt-1 text-sm text-muted-foreground">{t('overview.notFound.description')}</p>
    <Button class="mt-6" href={href('/')}>{t('overview.notFound.back')}</Button>
  </div>
</div>
