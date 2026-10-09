<script lang="ts">
  import PillTabs from '$lib/components/shared/PillTabs.svelte'
  import { databaseTabs, isActive } from '$lib/shell/nav'
  import { t } from '$lib/i18n/index.svelte'

  // Tables, SQL and Migrations share the Database pill; these tabs move
  // between them from inside each page.
  let { fill = false }: { fill?: boolean } = $props()
  const current = $derived(databaseTabs.find((tab) => isActive(tab.path))?.path ?? '/tables')
</script>

<PillTabs {fill} label={t('shell.projects.databaseTabs')} {current} tabs={databaseTabs.map((tab) => ({ path: tab.path, label: t(tab.title) }))} />
