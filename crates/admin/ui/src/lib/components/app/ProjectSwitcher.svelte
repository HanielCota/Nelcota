<script lang="ts">
  import { onMount } from 'svelte'
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu'
  import { Button } from '$lib/components/ui/button'
  import ChevronsUpDown from '@lucide/svelte/icons/chevrons-up-down'
  import Check from '@lucide/svelte/icons/check'
  import { toast } from 'svelte-sonner'
  import { api } from '$lib/api'
  import { openProject } from '$lib/projects'
  import { navigate } from '$lib/router.svelte'
  import type { ProjectsData } from '$lib/types'

  let data = $state<ProjectsData | null>(null)

  onMount(async () => {
    try {
      data = await api.get<ProjectsData>('/projects')
    } catch {
      data = null
    }
  })

  async function open(name: string) {
    const project = data?.projects.find((p) => p.name === name)
    if (!project || !data) return
    try {
      await openProject(project, data.sso)
    } catch (e) {
      toast.error((e as Error).message)
    }
  }
</script>

{#if data}
  <DropdownMenu.Root>
    <DropdownMenu.Trigger>
      {#snippet child({ props })}
        <Button variant="ghost" size="sm" class="gap-1.5 px-2 font-medium" {...props}>
          {data!.current}
          <ChevronsUpDown class="size-3.5 text-muted-foreground" />
        </Button>
      {/snippet}
    </DropdownMenu.Trigger>
    <DropdownMenu.Content align="start" class="w-60">
      <DropdownMenu.Label class="text-xs font-normal text-muted-foreground">
        Projetos{data.sso ? '' : ' (cada um pede o próprio login)'}
      </DropdownMenu.Label>
      {#each data.projects as project (project.name)}
        <DropdownMenu.Item disabled={!project.url && !project.current} onclick={() => open(project.name)}>
          <span class="flex-1 truncate">{project.name}</span>
          {#if project.current}<Check class="size-3.5" />{/if}
        </DropdownMenu.Item>
      {/each}
      <DropdownMenu.Separator />
      <DropdownMenu.Item onclick={() => navigate('/projects')}>Todos os projetos</DropdownMenu.Item>
    </DropdownMenu.Content>
  </DropdownMenu.Root>
  <span class="text-muted-foreground/60">/</span>
{/if}
