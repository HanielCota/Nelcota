<script lang="ts">
  import { access } from '$lib/shared/schema/plain'
  import { technical } from '$lib/shared/schema/technical.svelte'
  import { intlLocale, t } from '$lib/i18n/index.svelte'

  let { grants }: { grants: string[] } = $props()

  // What the role may do, in words; the raw grants with technical details on.
  const plain = $derived.by(() => {
    const { level, verbs } = access(grants)
    if (level !== 'partial') return t(`policies.plain.access.${level}`)
    const list = new Intl.ListFormat(intlLocale(), { type: 'conjunction' })
    return t('policies.plain.access.partial', { verbs: list.format(verbs.map((verb) => t(`policies.plain.verbs.${verb}`))) })
  })
  const raw = $derived(grants.length ? grants.map((g) => g.toLowerCase()).join(', ') : '—')
</script>

{#if technical.on}
  <span class="font-mono text-xs text-muted-foreground">{raw}</span>
{:else}
  <span class={['text-sm', grants.length ? 'text-foreground' : 'text-muted-foreground']} title={raw}>{plain}</span>
{/if}
