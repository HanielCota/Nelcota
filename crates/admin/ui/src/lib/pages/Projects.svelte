<script lang="ts">
  import { onMount } from 'svelte'
  import { Skeleton } from '$lib/components/ui/skeleton'
  import RefreshCw from '@lucide/svelte/icons/refresh-cw'
  import ArrowUpRight from '@lucide/svelte/icons/arrow-up-right'
  import Boxes from '@lucide/svelte/icons/boxes'
  import Box from '@lucide/svelte/icons/box'
  import Terminal from '@lucide/svelte/icons/terminal'
  import { Button } from '$lib/components/ui/button'
  import { toast } from 'svelte-sonner'
  import PageHeader from '$lib/components/app/PageHeader.svelte'
  import Callout from '$lib/components/app/Callout.svelte'
  import { api } from '$lib/api'
  import { openProject } from '$lib/projects'
  import type { ProjectStatus, ProjectsData } from '$lib/types'

  let projects = $state<ProjectStatus[] | null>(null)
  let sso = $state(false)
  let error = $state('')
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
      error = (e as Error).message
    } finally {
      refreshing = false
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

<div class="mx-auto max-w-6xl px-4 py-8 sm:px-6 lg:px-10 lg:py-12">
  <PageHeader
    title="Projetos"
    icon={Boxes}
    description={projects
      ? `${projects.length} ${projects.length === 1 ? 'projeto' : 'projetos'} neste host. ${sso ? 'Login único: os painéis abrem sem pedir senha.' : 'Cada painel pede o próprio login.'}`
      : 'Projetos que rodam neste host.'}
  >
    {#snippet actions()}
      <Button variant="outline" onclick={load} disabled={refreshing}>
        <RefreshCw class={refreshing ? 'animate-spin' : ''} />Atualizar
      </Button>
    {/snippet}
  </PageHeader>

  {#if error}
    <Callout variant="danger" title="Não foi possível listar os projetos">{error}</Callout>
  {:else if !projects}
    <div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
      {#each [0, 1, 2] as i (i)}<Skeleton class="h-48 rounded-xl" />{/each}
    </div>
  {:else}
    <div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
      {#each projects as project (project.name)}
        <div
          class={[
            'flex flex-col rounded-xl border bg-card p-6 shadow-card transition-[border-color,box-shadow] duration-200',
            project.current ? 'border-brand/40 ring-1 ring-brand/15' : 'hover:border-border-strong hover:shadow-raised',
          ]}
        >
          <div class="flex items-start gap-3">
            <span
              class={[
                'grid size-10 shrink-0 place-items-center rounded-lg border',
                project.current ? 'border-brand/20 bg-brand-soft text-brand' : 'bg-muted/60 text-muted-foreground',
              ]}
            >
              <Box class="size-5" strokeWidth={1.75} />
            </span>
            <div class="min-w-0 flex-1">
              <p class="truncate text-base font-semibold">{project.name}</p>
              <p class="mt-0.5 truncate font-mono text-xs text-muted-foreground">{host(project.url)}</p>
            </div>
            {#if project.current}
              <span
                class="shrink-0 rounded-full border border-brand/30 bg-brand-soft px-2.5 py-0.5 text-2xs font-semibold text-brand"
                >Atual</span
              >
            {/if}
          </div>

          <div class="mt-6 flex items-center gap-2 border-t pt-4 text-xs">
            {#if project.healthy}
              <span
                class="inline-flex items-center gap-1.5 rounded-full border border-brand/25 bg-brand-soft px-2.5 py-1 font-semibold text-brand"
              >
                <span class="size-1.5 rounded-full bg-brand shadow-[0_0_0_3px] shadow-brand/20"></span>No ar
              </span>
              {#if project.latency_ms !== null}
                <span class="font-mono text-muted-foreground tabular-nums">{project.latency_ms} ms</span>
              {/if}
            {:else}
              <span
                class="inline-flex items-center gap-1.5 rounded-full border border-destructive/25 bg-destructive/10 px-2.5 py-1 font-semibold text-destructive"
              >
                <span class="size-1.5 rounded-full bg-destructive"></span>Fora do ar
              </span>
            {/if}
            {#if project.version}<span class="ml-auto font-mono text-muted-foreground">{project.version}</span>{/if}
          </div>

          {#if !project.current && project.url}
            <Button variant="outline" class="mt-4 w-full" onclick={() => open(project)}>
              Abrir painel<ArrowUpRight />
            </Button>
          {/if}
        </div>
      {/each}
    </div>

    <div class="mt-8 flex gap-4 rounded-xl border border-dashed bg-card/40 p-6">
      <span class="grid size-10 shrink-0 place-items-center rounded-lg border bg-card text-muted-foreground shadow-card">
        <Terminal class="size-5" strokeWidth={1.75} />
      </span>
      <div class="min-w-0 text-sm">
        <p class="font-semibold">Novo projeto</p>
        <p class="mt-1 text-muted-foreground">
          Rode <code class="rounded bg-muted px-1.5 py-0.5 text-xs text-foreground">nelcota init --project nome</code>
          (subdomínio) ou
          <code class="rounded bg-muted px-1.5 py-0.5 text-xs text-foreground">nelcota init api.dominio.com</code>.
        </p>
      </div>
    </div>
  {/if}
</div>
