<script lang="ts">
  import { onMount } from 'svelte'
  import { Skeleton } from '$lib/components/ui/skeleton'
  import PageHeader from '$lib/components/shared/PageHeader.svelte'
  import PillTabs from '$lib/components/shared/PillTabs.svelte'
  import CodeBlock from '$lib/components/shared/CodeBlock.svelte'
  import LoadError from '$lib/components/shared/LoadError.svelte'
  import { RemoteResource } from '$lib/remote-resource.svelte'
  import { api } from '$lib/api'
  import { minutes, signInOptions } from '$lib/features/sign-in/sign-in-options'
  import type { SignIn } from '$lib/types'
  import { errorMessage, t } from '$lib/i18n/index.svelte'

  const resource = new RemoteResource<SignIn>()
  const settings = $derived(resource.data)
  const options = $derived(settings ? signInOptions(settings) : [])
  const providers = $derived(settings && (settings.providers.google || settings.providers.github))

  async function load() {
    await resource.load((signal) => api.get<SignIn>('/sign-in', { signal }))
  }
  onMount(() => {
    void load()
    return () => resource.cancel()
  })
</script>

<div class="grid gap-8 px-4 pt-2 pb-12 *:max-w-3xl *:min-w-0 sm:px-6 lg:px-8">
  <PageHeader title={t('signIn.title')} description={t('signIn.description')} />
  <div class="-mt-6"><PillTabs label={t('shell.pages.users')} current={'/sign-in'} tabs={[{ path: '/users', label: t('shell.pages.users') }, { path: '/sign-in', label: t('shell.pages.userSignIn') }]} /></div>

  {#if resource.error}
    <LoadError message={errorMessage(resource.error)} onretry={load} busy={resource.loading} />
  {:else if !settings}
    <Skeleton class="h-80" />
  {:else}
    <section class="-mt-6 grid gap-4">
      <p class="text-sm text-muted-foreground">{t('signIn.fromEnv')}</p>
      <ul class="divide-y rounded-3xl bg-card">
        {#each options as option (option.id)}
          <li class="grid gap-1 px-4 py-3.5">
            <div class="flex items-baseline justify-between gap-4">
              <h2 class="text-sm font-medium">{t(`signIn.options.${option.id}.title`)}</h2>
              <span class={['shrink-0 text-sm', option.on ? 'font-medium text-foreground' : 'text-muted-foreground']}>
                {option.on ? t('signIn.on') : t('signIn.off')}
              </span>
            </div>
            <p class="text-sm text-muted-foreground">{t(`signIn.options.${option.id}.text`)}</p>
            {#if option.variables.length}
              <p class="mt-1 flex flex-wrap items-baseline gap-x-2 gap-y-1 text-xs text-muted-foreground">
                {t('signIn.turnOn')}
                {#each option.variables as variable (variable)}<code class="text-foreground">{variable}</code>{/each}
              </p>
            {/if}
          </li>
        {/each}
      </ul>
    </section>

    {#if providers}
      <section class="grid gap-4">
        <h2 class="text-base font-semibold">{t('signIn.providers.title')}</h2>
        {#if settings.callback_url}
          <div class="grid gap-2">
            <p class="text-sm text-muted-foreground">{t('signIn.providers.callback')}</p>
            <CodeBlock code={settings.callback_url} label={t('signIn.providers.copyCallback')} />
          </div>
        {/if}
        <div class="grid gap-2">
          <p class="text-sm text-muted-foreground">{t('signIn.providers.pages')}</p>
          {#if settings.redirect_urls.length}
            <ul class="grid gap-1 text-sm">
              {#each settings.redirect_urls as url (url)}<li><code class="text-xs">{url}</code></li>{/each}
            </ul>
          {:else}
            <p class="text-sm text-warning">{t('signIn.providers.noPages')}</p>
          {/if}
        </div>
      </section>
    {/if}

    <section class="grid gap-3">
      <h2 class="text-base font-semibold">{t('signIn.session.title')}</h2>
      <ul class="grid gap-1.5 text-sm text-muted-foreground">
        <li>{t('signIn.session.access', { count: minutes(settings.access_ttl_secs) })}</li>
        <li>{t('signIn.session.refresh', { count: settings.refresh_ttl_days })}</li>
        <li>{t('signIn.session.rateLimit', { count: settings.rate_limit_per_minute })}</li>
      </ul>
    </section>
  {/if}
</div>
