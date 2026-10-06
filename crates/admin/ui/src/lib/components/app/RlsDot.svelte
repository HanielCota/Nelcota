<script lang="ts">
  import type { RlsState } from '$lib/types'

  let { state }: { state: RlsState } = $props()
</script>

<!-- Só aparece quando há algo a corrigir. -->
{#if state === 'danger' || state === 'warn'}
  <span class={['size-1.5 shrink-0 rounded-full', state === 'danger' ? 'bg-destructive' : 'bg-warning']}></span>
{:else}
  <span class="size-1.5 shrink-0"></span>
{/if}
