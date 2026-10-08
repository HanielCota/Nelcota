<script lang="ts">
  import mascot from '../../../assets/mascot.png'

  let {
    class: className = '',
    mark = false,
    size = 'md',
  }: { class?: string; mark?: boolean; size?: 'md' | 'lg' } = $props()
</script>

<!-- Nelcota mascot (the atlas's neutral pose, 128px for high-density screens). -->
<span class={['inline-flex items-center gap-2', className]}>
  <img src={mascot} alt="" class={['shrink-0 select-none', size === 'lg' ? 'size-9' : 'size-8']} draggable="false" />
  {#if !mark}<span class="text-lg font-semibold tracking-tight">Nelcota</span>{/if}
</span>
