<script lang="ts">
  import ShieldAlert from '@lucide/svelte/icons/shield-alert'
  import { t } from '$lib/i18n/index.svelte'
</script>

<!-- The most important fact about this screen: nothing is filtered by the
     access rules. Amber, since it is a warning, not decoration (D55). -->
<span
  class="inline-flex shrink-0 items-center gap-1.5 rounded-md border border-warning/30 bg-warning/10 px-2 py-1 text-xs font-medium text-warning"
  title={t('sql.editor.ownerNoteTitle')}
>
  <ShieldAlert class="size-3.5" aria-hidden="true" />{t('sql.editor.ownerNote')}<span class="hidden 2xl:inline">{t('sql.editor.ownerNoteMore')}</span>
</span>
