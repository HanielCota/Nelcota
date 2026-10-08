<script lang="ts">
  import { onMount } from 'svelte'
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu'
  import ChevronsUpDown from '@lucide/svelte/icons/chevrons-up-down'
  import Check from '@lucide/svelte/icons/check'
  import Boxes from '@lucide/svelte/icons/boxes'
  import { toast } from 'svelte-sonner'
  import { api } from '$lib/api'
  import { openProject } from '$lib/features/projects/projects'
  import { navigate } from '$lib/router.svelte'
  import { errorMessage, t } from '$lib/i18n/index.svelte'
  import type { ProjectsData } from '$lib/types'

  let { menuOpen = $bindable(false) }: { menuOpen?: boolean } = $props()

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
      toast.error(errorMessage(e))
    }
  }
</script>

{#if data}
  <DropdownMenu.Root bind:open={menuOpen}>
    <DropdownMenu.Trigger>
      {#snippet child({ props })}
        <button
          {...props}
          class="flex h-9 w-full min-w-0 cursor-pointer items-center justify-between gap-2 rounded-md px-2.5 text-sm font-medium whitespace-nowrap transition-colors hover:bg-sidebar-accent/60 aria-expanded:bg-sidebar-accent/60"
          title={t('shell.projects.switch')}
        >
          <span class="truncate">{data!.current}</span>
          <ChevronsUpDown class="size-4 shrink-0 text-muted-foreground" />
        </button>
      {/snippet}
    </DropdownMenu.Trigger>
    <DropdownMenu.Content align="start" class="w-72">
      <DropdownMenu.Label class="text-xs font-normal text-muted-foreground">
        {data.sso ? t('shell.projects.heading') : t('shell.projects.separateLogin')}
      </DropdownMenu.Label>
      {#each data.projects as project (project.name)}
        <DropdownMenu.Item disabled={!project.url && !project.current} onclick={() => open(project.name)}>
          <span class="flex-1 truncate">{project.name}</span>
          {#if project.current}<Check class="size-3.5 text-brand" />{/if}
        </DropdownMenu.Item>
      {/each}
      <DropdownMenu.Separator />
      <DropdownMenu.Item onclick={() => navigate('/projects')}><Boxes />{t('shell.projects.all')}</DropdownMenu.Item>
    </DropdownMenu.Content>
  </DropdownMenu.Root>
{/if}
