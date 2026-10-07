<script lang="ts">
  import type { RlsState } from '$lib/types'

  let { state }: { state: RlsState } = $props()
</script>

<!-- Só aparece quando há algo a corrigir. -->
{#if state === 'danger' || state === 'warn'}
  <span class={['size-2 shrink-0 rounded-full ring-2', state === 'danger' ? 'bg-destructive ring-destructive/20' : 'bg-warning ring-warning/20']}></span>
{:else}
  <span class="size-2 shrink-0"></span>
{/if}
