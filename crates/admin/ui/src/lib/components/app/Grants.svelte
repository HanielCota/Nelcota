<script lang="ts">
  let { grants }: { grants: string[] } = $props()
</script>

{#if grants.length === 0}
  <span class="text-muted-foreground">—</span>
{:else}
  <div class="flex flex-wrap gap-1">
    {#each grants as grant (grant)}
      <span
        class="rounded border bg-muted/60 px-1.5 py-px font-mono text-[10px] font-semibold text-muted-foreground"
        >{grant}</span
      >
    {/each}
  </div>
{/if}
