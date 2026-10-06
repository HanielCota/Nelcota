<script lang="ts">
  let { grants }: { grants: string[] } = $props()
</script>

<span class="font-mono text-xs text-muted-foreground">
  {grants.length ? grants.map((g) => g.toLowerCase()).join(', ') : '—'}
</span>
