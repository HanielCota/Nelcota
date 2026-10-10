<script lang="ts">
  import { onMount } from 'svelte'
  import { Skeleton } from '$lib/components/ui/skeleton'
  import ArrowUpRight from '@lucide/svelte/icons/arrow-up-right'
  import Boxes from '@lucide/svelte/icons/boxes'
  import RefreshCw from '@lucide/svelte/icons/refresh-cw'
  import Copy from '@lucide/svelte/icons/copy'
  import CircleCheck from '@lucide/svelte/icons/circle-check'
  import CircleAlert from '@lucide/svelte/icons/circle-alert'
  import CodeBlock from '$lib/components/shared/CodeBlock.svelte'
  import { Button } from '$lib/components/ui/button'
  import { toast } from 'svelte-sonner'
  import PageHeader from '$lib/components/shared/PageHeader.svelte'
  import LoadError from '$lib/components/shared/LoadError.svelte'
  import EmptyState from '$lib/components/shared/EmptyState.svelte'
  import { copyText } from '$lib/clipboard'
  import { RemoteResource } from '$lib/remote-resource.svelte'
  import { api } from '$lib/api'
  import { href } from '$lib/router.svelte'
  import { openProject } from '$lib/features/projects/projects'
  import type { ProjectStatus, ProjectsData } from '$lib/types'
  import { errorMessage, t } from '$lib/i18n/index.svelte'

  const resource = new RemoteResource<{ projects: ProjectStatus[]; sso: boolean }>()
  const projects = $derived(resource.data?.projects ?? null)
  const sso = $derived(resource.data?.sso ?? false)
  const failure = $derived(resource.error)
  const refreshing = $derived(resource.loading)
  async function load() {
    await resource.load(async signal => {
      const [list, status] = await Promise.all([
        api.get<ProjectsData>('/projects', { signal }),
        api.get<{ projects: ProjectStatus[] }>('/projects/status', { signal }),
      ])
      return { sso: list.sso, projects: status.projects.length ? status.projects : list.projects.map(p => ({ ...p, healthy: true, version: null, latency_ms: null })) }
    })
  }
  onMount(() => { void load(); return () => resource.cancel() })

  async function open(project: ProjectStatus) {
    try {
      await openProject(project, sso)
    } catch (e) {
      toast.error(errorMessage(e))
    }
  }

  const host = (url: string | null) => (url ? url.replace(/^https?:\/\//, '') : null)
</script>

<div class="mx-auto grid w-full max-w-page gap-6 px-4 pt-2 pb-12 *:min-w-0 sm:px-6 lg:px-8 [&>:first-child]:mb-0">
  <PageHeader
    title={t('projects.title')}
    description={projects ? (sso ? t('projects.sso') : t('projects.separateLogin')) : undefined}
  >
    {#snippet actions()}
      <Button variant="outline" onclick={load} disabled={refreshing}><RefreshCw data-icon="inline-start" class={refreshing ? 'animate-spin' : ''} aria-hidden="true" />{refreshing ? t('projects.refreshing') : t('common.refresh')}</Button>
    {/snippet}
  </PageHeader>

  {#if failure}<LoadError message={errorMessage(failure)} onretry={load} busy={refreshing} />{/if}
  {#if !projects && refreshing}
    <div class="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
      {#each [0, 1, 2] as i (i)}<Skeleton class="h-40 rounded-xl" />{/each}
    </div>
  {:else if projects}
    {#if projects.length === 0}<EmptyState icon={Boxes} title={t('projects.empty')} description={t('projects.emptyDescription')} />{/if}
    <div class="divide-y rounded-xl border bg-card">
      {#each projects as project (project.name)}
        <article class="grid gap-3 p-4 md:grid-cols-[minmax(0,1fr)_auto_auto] md:items-center">
          <div class="flex items-center gap-3">
            <div class="grid min-w-0 flex-1">
              <p class="truncate font-semibold">{project.name}</p>
              <p class="flex min-w-0 items-center gap-1 font-mono text-xs text-muted-foreground">
                <span class="truncate">{host(project.url) ?? (project.current ? location.host : '')}</span>
                {#if project.url}<Button variant="ghost" size="icon-xs" aria-label={t('projects.copyUrl', { name: project.name })} onclick={() => copyText(project.url!)}><Copy aria-hidden="true" /></Button>{/if}
              </p>
            </div>
            {#if project.current}<span class="shrink-0 text-xs font-medium text-muted-foreground">{t('projects.current')}</span>{/if}
          </div>

          <div class="flex items-center gap-2 text-sm md:px-4">
            {#if project.healthy}
              <CircleCheck class="size-4 text-brand" aria-hidden="true" />
              <span>{t('projects.up')}</span>
              {#if project.latency_ms !== null}
                <span class="text-muted-foreground tabular-nums">· {project.latency_ms} ms</span>
              {/if}
            {:else}
              <CircleAlert class="size-4 text-destructive" aria-hidden="true" />
              <span class="text-destructive">{t('projects.down')}</span>
            {/if}
            {#if project.version}<span class="ml-auto font-mono text-xs text-muted-foreground">v{project.version.replace(/^v/, '')}</span>{/if}
          </div>

          {#if project.current}
            <Button variant="outline" class="justify-self-start md:justify-self-end" href={href('/')}>{t('projects.back')}</Button>
          {:else if project.url}
            <Button variant="outline" class="justify-self-start md:justify-self-end" onclick={() => open(project)}>
              {t('projects.open')}<ArrowUpRight data-icon="inline-end" aria-hidden="true" />
            </Button>
          {/if}
        </article>
      {/each}

    </div>
    <details class="border-t pt-4">
      <summary class="cursor-pointer text-sm font-medium">{t('projects.newProject.title')}</summary>
      <div class="mt-4 grid gap-4 md:grid-cols-2">
        <div class="grid gap-2">
          <p class="text-sm text-muted-foreground">{t('projects.newProject.subdomain')}</p>
          <CodeBlock code={t('projects.newProject.subdomainCommand')} lang="sh" />
        </div>
        <div class="grid gap-2">
          <p class="text-sm text-muted-foreground">{t('projects.newProject.domain')}</p>
          <CodeBlock code={t('projects.newProject.domainCommand')} lang="sh" />
        </div>
      </div>
    </details>
  {/if}
</div>
