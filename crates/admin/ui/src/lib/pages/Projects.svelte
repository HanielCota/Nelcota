<script lang="ts">
  import { onMount } from 'svelte'
  import * as Table from '$lib/components/ui/table'
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

<div class="mx-auto max-w-5xl p-6">
  <PageHeader title="Projetos">
    {#snippet actions()}
      <Button variant="ghost" size="sm" onclick={load}>Atualizar</Button>
    {/snippet}
  </PageHeader>

  {#if error}
    <p class="text-sm text-destructive">{error}</p>
  {:else if !projects}
    <p class="text-sm text-muted-foreground">Carregando…</p>
  {:else}
    <p class="mb-3 text-sm text-muted-foreground">
      {projects.length} {projects.length === 1 ? 'projeto' : 'projetos'} neste host.
      {sso ? 'Login único: os painéis abrem sem pedir senha.' : 'Cada painel pede o próprio login.'}
    </p>
    <div class="rounded border">
      <Table.Root>
        <Table.Header>
          <Table.Row class="hover:bg-transparent">
            <Table.Head>Projeto</Table.Head>
            <Table.Head>Domínio</Table.Head>
            <Table.Head>Estado</Table.Head>
            <Table.Head>Versão</Table.Head>
            <Table.Head class="w-32"></Table.Head>
          </Table.Row>
        </Table.Header>
        <Table.Body>
          {#each projects as project (project.name)}
            <Table.Row>
              <Table.Cell class="font-medium">
                {project.name}
                {#if project.current}<span class="ml-1 text-xs font-normal text-muted-foreground">(este)</span>{/if}
              </Table.Cell>
              <Table.Cell class="font-mono text-xs">{host(project.url)}</Table.Cell>
              <Table.Cell>
                {#if project.healthy}
                  <span class="text-xs text-muted-foreground">
                    no ar{project.latency_ms !== null ? `, ${project.latency_ms} ms` : ''}
                  </span>
                {:else}
                  <span class="text-xs font-medium text-destructive">fora do ar</span>
                {/if}
              </Table.Cell>
              <Table.Cell class="font-mono text-xs text-muted-foreground">{project.version ?? '—'}</Table.Cell>
              <Table.Cell class="text-right">
                {#if !project.current && project.url}
                  <Button variant="ghost" size="sm" onclick={() => open(project)}>Abrir painel</Button>
                {/if}
              </Table.Cell>
            </Table.Row>
          {/each}
        </Table.Body>
      </Table.Root>
    </div>
    <p class="mt-4 text-sm text-muted-foreground">
      Novo projeto: <span class="font-mono">nelcota init --project nome</span> (subdomínio) ou
      <span class="font-mono">nelcota init api.dominio.com</span>.
    </p>
  {/if}
</div>
