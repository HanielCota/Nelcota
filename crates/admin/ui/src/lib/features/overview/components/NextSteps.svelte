<script lang="ts">
  import { Button } from '$lib/components/ui/button'
  import CircleCheck from '@lucide/svelte/icons/circle-check'
  import Circle from '@lucide/svelte/icons/circle'
  import { href } from '$lib/router.svelte'
  import { readText, write } from '$lib/local-storage'
  import { t } from '$lib/i18n/index.svelte'
  import type { Step } from '$lib/features/overview/next-steps'

  let { steps }: { steps: Step[] } = $props()

  // localStorage is per origin, and each project has its own: one key suffices.
  const KEY = 'nelcota.steps.hidden'
  let hidden = $state(readText(KEY, '') === 'yes')
  const done = $derived(steps.filter((step) => step.done).length)

  function hide() {
    hidden = true
    write(KEY, 'yes')
  }
</script>

{#if !hidden && done < steps.length}
  <section class="rounded-lg border bg-card" aria-labelledby="next-steps-title">
    <header class="flex items-center gap-3 border-b px-5 py-3">
      <h2 id="next-steps-title" class="text-base font-semibold">{t('overview.steps.title')}</h2>
      <span class="text-sm text-muted-foreground tabular-nums">{t('overview.steps.progress', { done, total: steps.length })}</span>
      <Button variant="ghost" size="sm" class="ml-auto" onclick={hide}>{t('overview.steps.hide')}</Button>
    </header>
    <ol class="divide-y">
      {#each steps as step (step.id)}
        <li class="flex flex-wrap items-center gap-x-4 gap-y-2 px-5 py-3.5">
          {#if step.done}
            <CircleCheck class="size-5 shrink-0 text-brand" aria-label={t('overview.steps.done')} />
          {:else}
            <Circle class="size-5 shrink-0 text-muted-foreground" aria-hidden="true" />
          {/if}
          <div class="min-w-0 flex-1">
            <p class={['font-medium', step.done && 'text-muted-foreground line-through decoration-1']}>{t(`overview.steps.${step.id}.label`)}</p>
            {#if !step.done}<p class="text-sm text-muted-foreground">{t(`overview.steps.${step.id}.hint`)}</p>{/if}
          </div>
          {#if !step.done}
            <Button variant="outline" size="sm" href={href(step.path)}>{t(`overview.steps.${step.id}.action`)}</Button>
          {/if}
        </li>
      {/each}
    </ol>
  </section>
{/if}
