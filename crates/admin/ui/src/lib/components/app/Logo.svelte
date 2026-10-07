<script lang="ts">
  let { class: className = '', mark = false }: { class?: string; mark?: boolean } = $props()
</script>

<span class={['inline-flex items-center gap-2', className]}>
  <svg viewBox="0 0 24 24" class="size-5 shrink-0" aria-hidden="true">
    <rect width="24" height="24" rx="6" class="fill-brand" />
    <path
      d="M7.5 17V7.5l9 9V7"
      fill="none"
      stroke="currentColor"
      stroke-width="2.2"
      stroke-linecap="round"
      stroke-linejoin="round"
      class="text-white dark:text-[#0b2a1c]"
    />
  </svg>
  {#if !mark}<span class="text-base font-medium tracking-tight">nelcota</span>{/if}
</span>
