<script lang="ts">
  let { grants }: { grants: string[] } = $props()
</script>

{#if grants.length}
  <span class="inline-flex flex-wrap gap-1">
    {#each grants as grant (grant)}
      <span class="rounded-md border border-border-strong bg-muted px-1.5 py-px font-mono text-2xs font-medium text-muted-foreground"
        >{grant.toLowerCase()}</span
      >
    {/each}
  </span>
{:else}
  <span class="text-xs text-muted-foreground">—</span>
{/if}
