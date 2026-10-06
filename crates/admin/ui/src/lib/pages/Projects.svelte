<script lang="ts">
  import { onMount } from 'svelte'
  import { Skeleton } from '$lib/components/ui/skeleton'
  import RefreshCw from '@lucide/svelte/icons/refresh-cw'
  import ArrowUpRight from '@lucide/svelte/icons/arrow-up-right'
  import { Button } from '$lib/components/ui/button'
  import { toast } from 'svelte-sonner'
  import PageHeader from '$lib/components/app/PageHeader.svelte'
  import { api } from '$lib/api'
  import { openProject } from '$lib/projects'
  import type { ProjectStatus, ProjectsData } from '$lib/types'

  let projects = $state<ProjectStatus[] | null>(null)
  let sso = $state(false)
  let error = $state('')

  async function load() {
    try {
      const [list, status] = await Promise.all([
        api.get<ProjectsData>('/projects'),
        api.get<{ projects: ProjectStatus[] }>('/projects/status'),
      ])
      sso = list.sso
      projects = status.projects.length ? status.projects : list.projects.map((p) => ({ ...p, healthy: true, version: null, latency_ms: null }))
    } catch (e) {
      error = (e as Error).message
    }
  }

  onMount(load)

  async function open(project: ProjectStatus) {
    try {
      await openProject(project, sso)
    } catch (e) {
      toast.error((e as Error).message)
    }
  }

  const host = (url: string | null) => (url ? url.replace(/^https?:\/\//, '') : '—')
</script>

<div class="mx-auto max-w-6xl px-6 py-10 lg:px-10">
  <PageHeader
    title="Projetos"
    description={projects
      ? `${projects.length} ${projects.length === 1 ? 'projeto' : 'projetos'} neste host. ${sso ? 'Login único: os painéis abrem sem pedir senha.' : 'Cada painel pede o próprio login.'}`
      : 'Projetos que rodam neste host.'}
  >
    {#snippet actions()}
      <Button variant="outline" size="sm" onclick={load}><RefreshCw />Atualizar</Button>
    {/snippet}
  </PageHeader>

  {#if error}
    <p class="rounded-md border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{error}</p>
  {:else if !projects}
    <div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
      {#each [0, 1, 2] as i (i)}<Skeleton class="h-40 rounded-lg" />{/each}
    </div>
  {:else}
    <div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
      {#each projects as project (project.name)}
        <div
          class={[
            'flex flex-col rounded-lg border bg-card p-5 transition-colors',
            project.current ? 'border-brand/40' : 'hover:border-border-strong',
          ]}
        >
          <div class="flex items-start justify-between gap-3">
            <div class="min-w-0">
              <p class="truncate font-medium">{project.name}</p>
              <p class="mt-0.5 truncate font-mono text-xs text-muted-foreground">{host(project.url)}</p>
            </div>
            {#if project.current}
              <span
                class="shrink-0 rounded-full border border-brand/30 bg-brand/10 px-2 py-px text-[11px] font-medium text-brand"
                >este</span
              >
            {/if}
          </div>

          <div class="mt-6 flex items-center gap-2 text-xs">
            {#if project.healthy}
              <span class="size-2 rounded-full bg-brand shadow-[0_0_0_3px] shadow-brand/20"></span>
              <span class="text-muted-foreground">
                No ar{project.latency_ms !== null ? ` · ${project.latency_ms} ms` : ''}
              </span>
            {:else}
              <span class="size-2 rounded-full bg-destructive shadow-[0_0_0_3px] shadow-destructive/20"></span>
              <span class="font-medium text-destructive">Fora do ar</span>
            {/if}
            {#if project.version}<span class="ml-auto font-mono text-muted-foreground">{project.version}</span>{/if}
          </div>

          {#if !project.current && project.url}
            <Button variant="outline" size="sm" class="mt-4 w-full" onclick={() => open(project)}>
              Abrir painel<ArrowUpRight />
            </Button>
          {/if}
        </div>
      {/each}
    </div>

    <div class="mt-8 rounded-lg border border-dashed p-5 text-sm">
      <p class="font-medium">Novo projeto</p>
      <p class="mt-1 font-light text-muted-foreground">
        <code class="text-xs text-foreground">nelcota init --project nome</code> (subdomínio) ou
        <code class="text-xs text-foreground">nelcota init api.dominio.com</code>.
      </p>
    </div>
  {/if}
</div>
