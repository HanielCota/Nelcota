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
  import mascot from '../../../../assets/mascot.png'
  import { environment, health, watchHealth } from '$lib/shell/health.svelte'

  let { menuOpen = $bindable(false) }: { menuOpen?: boolean } = $props()

  let data = $state<ProjectsData | null>(null)

  // The project chip (D98): mascot, project name and where it runs, with a
  // dot for whether the server answers. It balances the account chip.
  const env = environment()
  const where = env.local ? t('shell.projects.environment.local') : t('shell.projects.environment.remote', { host: env.host })
  const status = $derived(t(`shell.projects.server.${health.status}`))

  onMount(async () => {
    watchHealth()
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

{#snippet chip(props: Record<string, unknown> = {})}
  <button
    {...props}
    type="button"
    class="flex h-14 w-full min-w-0 cursor-pointer items-center gap-3 rounded-full bg-nav p-1.5 sm:pr-4 text-left whitespace-nowrap transition-colors hover:bg-accent aria-expanded:bg-accent"
    title={`${t('shell.projects.switch')} · ${status}`}
    aria-label={`${t('shell.projects.switch')}: ${data?.current ?? 'Nelcota'} · ${status}`}
  >
    <span class="relative grid size-11 shrink-0 place-items-center">
      <img src={mascot} alt="" class="size-10 select-none" draggable="false" />
      <span
        class={[
          'absolute right-0 bottom-0 size-3 rounded-full ring-[3px] ring-nav',
          health.status === 'online' ? 'bg-brand' : health.status === 'offline' ? 'bg-destructive' : 'bg-muted-foreground',
        ]}
        role="img"
        aria-label={status}
      ></span>
    </span>
    <span class="hidden min-w-0 flex-1 flex-col leading-tight sm:flex lg:hidden xl:flex">
      <span class="truncate text-sm font-semibold">{data?.current ?? 'Nelcota'}</span>
      <span class={['truncate text-xs', health.status === 'offline' ? 'text-destructive' : 'text-muted-foreground']}>{health.status === 'offline' ? status : where}</span>
    </span>
    <ChevronsUpDown class="ml-auto hidden size-4 shrink-0 text-muted-foreground sm:block" aria-hidden="true" />
  </button>
{/snippet}

{#if data}
  <DropdownMenu.Root bind:open={menuOpen}>
    <DropdownMenu.Trigger>
      {#snippet child({ props })}{@render chip(props)}{/snippet}
    </DropdownMenu.Trigger>
    <DropdownMenu.Content align="start" class="min-w-72">
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
{:else}
  {@render chip()}
{/if}
