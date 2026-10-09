<script lang="ts">
  import { onMount } from 'svelte'
  import { Skeleton } from '$lib/components/ui/skeleton'
  import ArrowUpRight from '@lucide/svelte/icons/arrow-up-right'
  import Boxes from '@lucide/svelte/icons/boxes'
  import RefreshCw from '@lucide/svelte/icons/refresh-cw'
  import Copy from '@lucide/svelte/icons/copy'
  import CircleCheck from '@lucide/svelte/icons/circle-check'
  import CircleAlert from '@lucide/svelte/icons/circle-alert'
  import { Button } from '$lib/components/ui/button'
  import { toast } from 'svelte-sonner'
  import PageHeader from '$lib/components/shared/PageHeader.svelte'
  import LoadError from '$lib/components/shared/LoadError.svelte'
  import EmptyState from '$lib/components/shared/EmptyState.svelte'
  import { copyText } from '$lib/clipboard'
  import { RemoteResource } from '$lib/remote-resource.svelte'
  import { api } from '$lib/api'
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

  const host = (url: string | null) => (url ? url.replace(/^https?:\/\//, '') : '—')
</script>

<div class="px-4 pt-2 pb-12 sm:px-6 lg:px-8">
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
      {#each [0, 1, 2] as i (i)}<Skeleton class="h-36 rounded-lg" />{/each}
    </div>
  {:else if projects}
    {#if projects.length === 0}<EmptyState icon={Boxes} title={t('projects.empty')} description={t('projects.emptyDescription')} />{/if}
    <div class="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
      {#each projects as project (project.name)}
        <div class={['flex flex-col rounded-3xl bg-card p-5', project.current && 'border-brand/40']}>
          <div class="flex items-baseline justify-between gap-3">
            <p class="flex min-w-0 items-center gap-2 font-medium"><Boxes class="size-4 shrink-0 text-muted-foreground" aria-hidden="true" /><span class="truncate">{project.name}</span></p>
            {#if project.current}<span class="shrink-0 text-xs text-muted-foreground">{t('projects.current')}</span>{/if}
          </div>
          <div class="mt-1 flex min-w-0 items-center gap-1"><p class="min-w-0 flex-1 truncate font-mono text-xs text-muted-foreground">{host(project.url)}</p>{#if project.url}<Button variant="ghost" size="icon-xs" aria-label={t('projects.copyUrl', { name: project.name })} onclick={() => copyText(project.url!)}><Copy aria-hidden="true" /></Button>{/if}</div>

          <div class="mt-5 flex items-center gap-2 text-sm">
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
            {#if project.version}<span class="ml-auto font-mono text-xs text-muted-foreground">{project.version}</span>{/if}
          </div>

          {#if !project.current && project.url}
            <Button variant="outline" class="mt-4 w-full" onclick={() => open(project)}>
              {t('projects.open')}<ArrowUpRight />
            </Button>
          {/if}
        </div>
      {/each}
    </div>

    <p class="mt-8 text-sm text-muted-foreground">
      {t('projects.newProject.before')}
      <code class="text-xs text-foreground">{t('projects.newProject.subdomainCommand')}</code>
      {t('projects.newProject.middle')}
      <code class="text-xs text-foreground">{t('projects.newProject.domainCommand')}</code>.
    </p>
  {/if}
</div>
