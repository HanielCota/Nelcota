<script lang="ts">
  import { onMount } from 'svelte'
  import { Skeleton } from '$lib/components/ui/skeleton'
  import ArrowUpRight from '@lucide/svelte/icons/arrow-up-right'
  import { Button } from '$lib/components/ui/button'
  import { toast } from 'svelte-sonner'
  import PageHeader from '$lib/components/app/PageHeader.svelte'
  import { api } from '$lib/api'
  import { openProject } from '$lib/projects'
  import type { ProjectStatus, ProjectsData } from '$lib/types'
  import { errorMessage, t } from '$lib/i18n/index.svelte'

  let projects = $state<ProjectStatus[] | null>(null)
  let sso = $state(false)
  let failure = $state<unknown>(null)
  let refreshing = $state(false)

  async function load() {
    refreshing = true
    try {
      const [list, status] = await Promise.all([
        api.get<ProjectsData>('/projects'),
        api.get<{ projects: ProjectStatus[] }>('/projects/status'),
      ])
      sso = list.sso
      projects = status.projects.length ? status.projects : list.projects.map((p) => ({ ...p, healthy: true, version: null, latency_ms: null }))
    } catch (e) {
      failure = e
    } finally {
      refreshing = false
    }
  }

  onMount(load)

  async function open(project: ProjectStatus) {
    try {
      await openProject(project, sso)
    } catch (e) {
      toast.error(errorMessage(e))
    }
  }

  const host = (url: string | null) => (url ? url.replace(/^https?:\/\//, '') : '—')
</script>

<div class="mx-auto max-w-6xl px-4 py-8 sm:px-6 lg:px-10 lg:py-10">
  <PageHeader
    title={t('projects.title')}
    description={projects ? (sso ? t('projects.sso') : t('projects.separateLogin')) : undefined}
  >
    {#snippet actions()}
      <Button variant="outline" onclick={load} disabled={refreshing}>{refreshing ? t('projects.refreshing') : t('common.refresh')}</Button>
    {/snippet}
  </PageHeader>

  {#if failure}
    <p class="text-sm text-destructive">{errorMessage(failure)}</p>
  {:else if !projects}
    <div class="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
      {#each [0, 1, 2] as i (i)}<Skeleton class="h-36 rounded-lg" />{/each}
    </div>
  {:else}
    <div class="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
      {#each projects as project (project.name)}
        <div class={['flex flex-col rounded-lg border bg-card p-5', project.current && 'border-brand/40']}>
          <div class="flex items-baseline justify-between gap-3">
            <p class="truncate font-medium">{project.name}</p>
            {#if project.current}<span class="shrink-0 text-xs text-muted-foreground">{t('projects.current')}</span>{/if}
          </div>
          <p class="mt-0.5 truncate font-mono text-xs text-muted-foreground">{host(project.url)}</p>

          <div class="mt-5 flex items-center gap-2 text-sm">
            {#if project.healthy}
              <span class="size-2 rounded-full bg-brand"></span>
              <span>{t('projects.up')}</span>
              {#if project.latency_ms !== null}
                <span class="text-muted-foreground tabular-nums">· {project.latency_ms} ms</span>
              {/if}
            {:else}
              <span class="size-2 rounded-full bg-destructive"></span>
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
