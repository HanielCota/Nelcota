<script lang="ts">
  import mascot from '../../../assets/mascot.png'

  let {
    class: className = '',
    mark = false,
    size = 'md',
  }: { class?: string; mark?: boolean; size?: 'md' | 'lg' } = $props()
</script>

<!-- Mascote do Nelcota (recorte da pose neutra do atlas, 128px para telas de alta densidade). -->
<span class={['inline-flex items-center gap-2', className]}>
  <img src={mascot} alt="" class={['shrink-0 select-none', size === 'lg' ? 'size-9' : 'size-8']} draggable="false" />
  {#if !mark}<span class="text-lg font-semibold tracking-tight">Nelcota</span>{/if}
</span>
