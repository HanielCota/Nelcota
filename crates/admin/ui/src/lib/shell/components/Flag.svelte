<script lang="ts">
  import type { Locale } from '$lib/i18n/index.svelte'

  let { locale, class: className = '' }: { locale: Locale; class?: string } = $props()

  // US stripes, red ones only (the white ones are the background).
  const RED_STRIPES = [0, 2, 4, 6, 8, 10, 12].map((i) => (i * 24) / 13)
  // A few stars on the canton: at this size they read as dots.
  const STARS = [
    [2.2, 2],
    [5.4, 2],
    [8.6, 2],
    [3.8, 4.5],
    [7, 4.5],
    [2.2, 7],
    [5.4, 7],
    [8.6, 7],
    [3.8, 9.5],
    [7, 9.5],
  ]
</script>

<!-- A round flag for a panel language, drawn here because Windows shows flag
     emoji as two letters. Decorative: the control around it names the language. -->
<svg viewBox="0 0 24 24" class={['shrink-0 rounded-full', className]} aria-hidden="true">
  <clipPath id="flag-{locale}"><circle cx="12" cy="12" r="12" /></clipPath>
  <g clip-path="url(#flag-{locale})">
    {#if locale === 'pt-BR'}
      <rect width="24" height="24" fill="#009c3b" />
      <polygon points="12,3.2 22.4,12 12,20.8 1.6,12" fill="#ffdf00" />
      <circle cx="12" cy="12" r="4.8" fill="#002776" />
      <path d="M7.4 11.1q4.8-1.7 9.3 1.4" fill="none" stroke="#fff" stroke-width="0.9" />
    {:else}
      <rect width="24" height="24" fill="#fff" />
      {#each RED_STRIPES as y (y)}<rect {y} width="24" height={24 / 13} fill="#b22234" />{/each}
      <rect width="11" height={(24 / 13) * 7} fill="#3c3b6e" />
      {#each STARS as [cx, cy] (`${cx},${cy}`)}<circle {cx} {cy} r="0.6" fill="#fff" />{/each}
    {/if}
  </g>
  <!-- A hairline edge, so the white of a flag does not melt into a white pill. -->
  <circle cx="12" cy="12" r="11.6" fill="none" stroke="currentColor" stroke-opacity="0.15" stroke-width="0.8" />
</svg>
