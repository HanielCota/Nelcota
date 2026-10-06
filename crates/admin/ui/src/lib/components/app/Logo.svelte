<script lang="ts">
  let { class: className = 'size-7' }: { class?: string } = $props()
</script>

<svg viewBox="0 0 32 32" class={className} aria-hidden="true">
  <defs>
    <linearGradient id="nelcota-logo" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#3ecf8e" />
      <stop offset="1" stop-color="#1f7a52" />
    </linearGradient>
  </defs>
  <rect width="32" height="32" rx="8" fill="url(#nelcota-logo)" />
  <path d="M10 23V9h3.2l6 9V9H22v14h-3.2l-6-9v9z" fill="#0f1f17" />
</svg>
