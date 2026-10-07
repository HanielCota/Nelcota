<script lang="ts">
  import { onMount, type Snippet } from 'svelte'
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu'
  import ChevronsUpDown from '@lucide/svelte/icons/chevrons-up-down'
  import Check from '@lucide/svelte/icons/check'
  import Boxes from '@lucide/svelte/icons/boxes'
  import { toast } from 'svelte-sonner'
  import { api } from '$lib/api'
  import { openProject } from '$lib/projects'
  import { navigate } from '$lib/router.svelte'
  import type { ProjectsData } from '$lib/types'

  let { slash }: { slash: Snippet } = $props()

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
        <button
          {...props}
          class="flex h-9 min-w-0 cursor-pointer items-center gap-2 rounded-md px-2.5 text-sm font-medium transition-colors hover:bg-accent aria-expanded:bg-accent"
        >
          <span class="truncate">{data!.current}</span>
          <ChevronsUpDown class="size-4 shrink-0 text-muted-foreground" />
        </button>
      {/snippet}
    </DropdownMenu.Trigger>
    <DropdownMenu.Content align="start" class="w-72">
      <DropdownMenu.Label class="text-xs font-normal text-muted-foreground">
        Projetos{data.sso ? '' : ' (cada um pede o próprio login)'}
      </DropdownMenu.Label>
      {#each data.projects as project (project.name)}
        <DropdownMenu.Item disabled={!project.url && !project.current} onclick={() => open(project.name)}>
          <span class="flex-1 truncate">{project.name}</span>
          {#if project.current}<Check class="size-3.5 text-brand" />{/if}
        </DropdownMenu.Item>
      {/each}
      <DropdownMenu.Separator />
      <DropdownMenu.Item onclick={() => navigate('/projects')}><Boxes />Todos os projetos</DropdownMenu.Item>
    </DropdownMenu.Content>
  </DropdownMenu.Root>
  {@render slash()}
{/if}
