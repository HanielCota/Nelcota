<script lang="ts">
  import mascot from '../../../assets/mascot.png'

  let {
    class: className = '',
    mark = false,
    size = 'md',
  }: { class?: string; mark?: boolean; size?: 'md' | 'lg' | 'xl' } = $props()
</script>

<!-- Nelcota mascot (the atlas's neutral pose, 128px for high-density screens). -->
<span class={['inline-flex items-center gap-2', className]}>
  <img src={mascot} alt="" width="128" height="128" class={['shrink-0 select-none', size === 'xl' ? 'size-14' : size === 'lg' ? 'size-9' : 'size-8']} draggable="false" />
  {#if !mark}<span class={['font-semibold tracking-tight', size === 'xl' ? 'text-2xl' : 'text-lg']} translate="no">Nelcota</span>{/if}
</span>
