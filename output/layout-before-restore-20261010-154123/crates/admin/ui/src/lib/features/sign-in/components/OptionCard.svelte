<script lang="ts">
  import type { Component } from 'svelte'
  import ChevronDown from '@lucide/svelte/icons/chevron-down'
  import KeyRound from '@lucide/svelte/icons/key-round'
  import MailCheck from '@lucide/svelte/icons/mail-check'
  import RotateCcwKey from '@lucide/svelte/icons/rotate-ccw-key'
  import WandSparkles from '@lucide/svelte/icons/wand-sparkles'
  import CodeBlock from '$lib/components/shared/CodeBlock.svelte'
  import BrandMark from '$lib/features/sign-in/components/BrandMark.svelte'
  import { envSnippet, type OptionId, type SignInOption } from '$lib/features/sign-in/sign-in-options'
  import { t } from '$lib/i18n/index.svelte'

  // One way to sign in: what it does, whether it is on and, while it is off,
  // the .env lines that turn it on, folded away until asked for.
  let { option }: { option: SignInOption } = $props()
  let open = $state(false)

  const ICONS: Partial<Record<OptionId, Component>> = { password: KeyRound, confirmation: MailCheck, recovery: RotateCcwKey, magicLink: WandSparkles }
  const Icon = $derived(ICONS[option.id])
  const panel = `signin-${Math.random().toString(36).slice(2, 8)}`
</script>

<article class="grid grid-cols-[auto_minmax(0,1fr)_auto] gap-x-3 gap-y-2 border-b py-4">
  <span class="grid size-5 place-items-center text-muted-foreground" aria-hidden="true">
    {#if Icon}<Icon class="size-[18px]" />{:else if option.id === 'google' || option.id === 'github'}<BrandMark brand={option.id} />{/if}
  </span>
  <div class="grid gap-1">
    <h3 class="text-base font-semibold">{t(`signIn.options.${option.id}.title`)}</h3>
    <p class="text-sm text-muted-foreground">{t(`signIn.options.${option.id}.text`)}</p>
  </div>
  <span class={['flex items-center gap-1.5 self-start text-xs font-medium', option.on ? 'text-brand' : 'text-muted-foreground']}>
    <span class={['size-1.5 rounded-full', option.on ? 'bg-brand' : 'bg-muted-foreground/60']} aria-hidden="true"></span>
    {option.on ? t('signIn.on') : t('signIn.off')}
  </span>

  {#if option.variables.length}
    <div class="col-span-2 col-start-2 grid gap-3">
      <button
        type="button"
        class="flex w-fit cursor-pointer items-center gap-1 text-sm font-medium text-foreground/80 hover:text-foreground"
        aria-expanded={open}
        aria-controls={panel}
        onclick={() => (open = !open)}
      >
        {t('signIn.howTo')}
        <ChevronDown class={['size-4 transition-transform', open && 'rotate-180']} aria-hidden="true" />
      </button>
      {#if open}
        <div id={panel} class="grid gap-2">
          <p class="text-xs text-muted-foreground">{t('signIn.envHint')}</p>
          <CodeBlock code={envSnippet(option.variables)} label={t('signIn.copyEnv')} lang="env" wrap />
        </div>
      {/if}
    </div>
  {/if}
</article>
