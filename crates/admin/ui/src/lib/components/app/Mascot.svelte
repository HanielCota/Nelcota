<script lang="ts" module>
  import neutral from '../../../assets/mascot/neutral.png'
  import wave from '../../../assets/mascot/wave.png'
  import eyesClosed from '../../../assets/mascot/eyes-closed.png'
  import sad from '../../../assets/mascot/sad.png'

  const frames = { neutral, wave, eyesClosed, sad }
  export type Pose = keyof typeof frames
</script>

<script lang="ts">
  import { EYES, EYE_ANGLE, FRAME_SIZE, approach, gazeAt, pupilOffset, type Point } from '$lib/mascot'
  import { cn } from '$lib/utils'

  let {
    pose = 'neutral',
    lookAt = null,
    class: className = '',
  }: {
    pose?: Pose
    /** Ponto da tela para onde olhar (ex.: o texto sendo digitado); sem ele, segue o ponteiro. */
    lookAt?: Point | null
    class?: string
  } = $props()

  const ahead = () => EYES.map(() => ({ x: 0, y: 0 }))

  let root = $state<HTMLDivElement>()
  let pointer = $state<Point | null>(null)
  // Direção atual de cada olho, suavizada a cada quadro.
  let gaze = $state<Point[]>(ahead())

  const target = $derived(lookAt ?? pointer)
  const open = $derived(pose !== 'eyesClosed')
  const reduced = matchMedia('(prefers-reduced-motion: reduce)')

  // Anima só enquanto o olhar não chegou ao alvo; parado, não gasta quadros.
  let frame = 0
  function tick() {
    frame = 0
    if (!root) return
    const desired = target ? gazeAt(root.getBoundingClientRect(), target) : ahead()
    const next = gaze.map((g, i) => approach(g, desired[i], reduced.matches ? 1 : 0.18))
    const moving = next.some((g, i) => Math.hypot(g.x - desired[i].x, g.y - desired[i].y) > 0.002)
    gaze = moving ? next : desired
    if (moving) frame = requestAnimationFrame(tick)
  }

  $effect(() => {
    void target
    if (!frame) frame = requestAnimationFrame(tick)
  })
  $effect(() => () => cancelAnimationFrame(frame))

  function onMove(e: PointerEvent) {
    pointer = { x: e.clientX, y: e.clientY }
  }
  // Ponteiro saiu da janela: volta a olhar para a frente.
  function onLeave(e: MouseEvent) {
    if (!e.relatedTarget) pointer = null
  }
</script>

<svelte:window onpointermove={onMove} />
<svelte:document onmouseout={onLeave} />

<!-- cn: quem usa pode trocar o `relative` por `absolute` sem conflito de classes. -->
<div bind:this={root} class={cn('relative select-none', className)} aria-hidden="true">
  <!-- Todas as poses ficam carregadas: trocar de expressão não pisca. -->
  {#each Object.entries(frames) as [name, src] (name)}
    <img {src} alt="" draggable="false" class={['absolute inset-0 size-full', pose !== name && 'invisible']} />
  {/each}

  <!-- Olhos vetoriais por cima dos do desenho: só a pupila se move. -->
  {#if open}
    <svg viewBox="0 0 {FRAME_SIZE} {FRAME_SIZE}" class="absolute inset-0 size-full">
      {#each EYES as eye, i (i)}
        {@const p = pupilOffset(gaze[i], eye)}
        <g transform="translate({eye.x} {eye.y}) rotate({EYE_ANGLE})">
          <ellipse rx={eye.rx} ry={eye.ry} fill="#fcfaef" />
          <ellipse cx={p.x} cy={p.y} rx={eye.pupilRx} ry={eye.pupilRy} fill="#231f19" />
        </g>
      {/each}
    </svg>
  {/if}
</div>
