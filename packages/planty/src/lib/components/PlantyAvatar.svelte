<script lang="ts">
  import { scale } from 'svelte/transition';
  export type Mood = 'idle' | 'talking' | 'happy' | 'thinking' | 'moving';

  interface Props {
    x: number;
    y: number;
    mood?: Mood;
  }

  let { x = $bindable(0), y = $bindable(0), mood = 'idle' }: Props = $props();

  // ── Drag ─────────────────────────────────────────────────────────────
  let dragging = $state(false);
  let dragOffsetX = 0;
  let dragOffsetY = 0;

  function onPointerDown(e: PointerEvent) {
    if (e.button !== 0) return;
    dragging = true;
    dragOffsetX = e.clientX - x;
    dragOffsetY = e.clientY - y;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    e.preventDefault();
    e.stopPropagation();
  }

  function onPointerMove(e: PointerEvent) {
    if (!dragging) return;
    x = Math.max(Math.min(e.clientX - dragOffsetX, window.innerWidth - 45), 5);
    y = Math.max(Math.min(e.clientY - dragOffsetY, window.innerHeight - 75), 5);
  }

  function onPointerUp() {
    dragging = false;
  }

  const displayMood = $derived(dragging ? 'moving' : mood);

  let mouthOpen = $state(false);

  $effect(() => {
    if (displayMood !== 'talking') {
      mouthOpen = false;
      return;
    }
    const id = setInterval(() => {
      mouthOpen = !mouthOpen;
    }, 180);
    return () => clearInterval(id);
  });

  const MOUTH_DOWN =
    'M29.5 55L28 63L23 68.5L14 70.5L6.5 66L4 58.5L10.5 29L15 24H24L28 29.5L28.5 34L23 58L16.5 61.5L10.5 59.5L8.5 53.5';
  const MOUTH_UP =
    'M29.5 55L28 63L23 68.5L14 70.5L6.5 66L4 58.5L10.5 29L15 24H24L28 29.5L28.5 34L24 56.5L17.5 60L11.5 58L9.5 52';

  const bodyPath = $derived(
    (displayMood === 'talking' && mouthOpen) || displayMood === 'happy' ? MOUTH_DOWN : MOUTH_UP
  );

  // ── Cursor-tracking pupils ────────────────────────────────────────────
  // Avatar screen positions of each eye centre (SVG natural size 46×74)
  let cursorX = $state(-9999);
  let cursorY = $state(-9999);

  function onMouseMove(e: MouseEvent) {
    cursorX = e.clientX;
    cursorY = e.clientY;
  }

  function pupilOffset(cx: number, cy: number, eyeSvgX: number, eyeSvgY: number, maxPx = 2.8) {
    const ex = x + eyeSvgX;
    const ey = y + eyeSvgY;
    const dx = cx - ex;
    const dy = cy - ey;
    const dist = Math.sqrt(dx * dx + dy * dy);
    if (dist < 1) return { px: 0, py: 0 };
    // Ramp up to full offset over 120px of distance
    const t = Math.min(dist, 120) / 120;
    return { px: (dx / dist) * maxPx * t, py: (dy / dist) * maxPx * t };
  }

  const left = $derived(
    displayMood === 'talking' ? { px: 0, py: 0 } : pupilOffset(cursorX, cursorY, 9.5, 30.5)
  );
  const right = $derived(
    displayMood === 'talking' ? { px: 0, py: 0 } : pupilOffset(cursorX, cursorY, 31.5, 35.5)
  );
</script>

<svelte:window onmousemove={onMouseMove} />

<div
  class="avatar"
  role="button"
  tabindex="0"
  in:scale={{ duration: 400, delay: 300 }}
  class:mood-idle={displayMood === 'idle'}
  class:mood-thinking={displayMood === 'thinking'}
  class:mood-talking={displayMood === 'talking'}
  class:mood-happy={displayMood === 'happy'}
  class:mood-moving={displayMood === 'moving'}
  class:dragging
  style:left="{x}px"
  style:top="{y}px"
  onpointerdown={onPointerDown}
  onpointermove={onPointerMove}
  onpointerup={onPointerUp}
>
  <svg
    width="46"
    height="74"
    viewBox="0 0 46 74"
    fill="none"
    xmlns="http://www.w3.org/2000/svg"
    overflow="visible"
  >
    <!--
			Leaf hinge points (transform-box: fill-box):
			  leave-right  →  origin 0% 100%   (bottom-left of bbox)
			  leave-left   →  origin 100% 100% (bottom-right of bbox)
		-->
    <g class="leave-right">
      <path
        d="M26.9781 16.5596L22.013 23.2368L22.8082 25.306L35.2985 25.3849L43.7783 20.6393L45.8723 14.8213L35.7374 14.0864L26.9781 16.5596Z"
        fill="#4F7B41"
      />
      <path
        d="M27 16.5L22.013 23.2368L22.8082 25.306L29 21L36.5 17L45.8723 14.8213L36 14L27 16.5Z"
        fill="#406634"
      />
    </g>

    <g class="leave-left">
      <path
        d="M11.3107 19.2204L17.7636 24.7215L20.3207 25.3703L22.8257 13.0024L19.0993 2.99176L12.5794 1.95314e-05L10.0997 9.77364L11.3107 19.2204Z"
        fill="#4F7B41"
      />
      <path
        d="M11.3107 19.2204L17.7636 24.7215L20.3207 25.3703L16 17L13.5 8L12.5794 1.95314e-05L10.0997 9.77364L11.3107 19.2204Z"
        fill="#5E8751"
      />
    </g>

    <path class="body" d={bodyPath} stroke="#4F7B41" stroke-width="3" />

    <!-- Left eye — pupils translated toward cursor -->
    <g class="eye-left">
      <circle cx="9.5" cy="30.5" r="9.5" fill="white" />
      <g transform="translate({left.px} {left.py})">
        <circle class="pupil" cx="9.5" cy="30.5" r="6.5" fill="black" />
        <circle cx="10.5" cy="27.5" r="2.5" fill="white" />
      </g>
    </g>

    <!-- Right eye — pupils translated toward cursor -->
    <g class="eye-right">
      <circle cx="31.5" cy="35.5" r="9.5" fill="white" />
      <g transform="translate({right.px} {right.py})">
        <circle class="pupil" cx="30.5" cy="34.5" r="6.5" fill="black" />
        <circle cx="30.5" cy="31.5" r="2.5" fill="white" />
      </g>
    </g>
  </svg>
</div>

<style>
  /* ── Wrapper ─────────────────────────────────────────────────────── */
  .avatar {
  	position: absolute;
  	cursor: grab;
  	user-select: none;
  	pointer-events: auto;
  	filter: drop-shadow(0px 0px 10px black);
  	transition:
  		left 0.85s cubic-bezier(0.33, 1, 0.68, 1),
  		top 0.85s cubic-bezier(0.33, 1, 0.68, 1);
  }

  .dragging {
  	cursor: grabbing;
  	transition: none;
  }

  /* idle: steady vertical bob */
  @keyframes bob {
  	0%,
  	100% {
  		transform: translateY(0);
  	}
  	50% {
  		transform: translateY(-5px);
  	}
  }
  .mood-idle {
  	animation: bob 2.6s ease-in-out infinite;
  }
  .mood-happy {
  	animation: bob 1.8s ease-in-out infinite;
  }

  /* thinking: head tilted to the side — clearly different from idle */
  @keyframes think {
  	0%,
  	100% {
  		transform: rotate(-12deg) translateY(0);
  	}
  	50% {
  		transform: rotate(-12deg) translateY(-3px);
  	}
  }
  .mood-thinking {
  	animation: think 2.8s ease-in-out infinite;
  }

  /* talking: subtle head waggle */
  @keyframes waggle {
  	0%,
  	100% {
  		transform: rotate(0deg);
  	}
  	25% {
  		transform: rotate(-2deg) translateY(-1px);
  	}
  	75% {
  		transform: rotate(2deg) translateY(1px);
  	}
  }
  .mood-talking {
  	animation: waggle 0.3s ease-in-out infinite;
  }

  /* moving: forward-lean glide */
  @keyframes glide {
  	0%,
  	100% {
  		transform: translateY(0) rotate(-6deg);
  	}
  	50% {
  		transform: translateY(-8px) rotate(-4deg);
  	}
  }
  .mood-moving {
  	animation: glide 0.4s ease-in-out infinite;
  }

  /* ── Drop shadows ────────────────────────────────────────────────── */
  .body {
  	filter: drop-shadow(1px 1px 0 rgba(0, 0, 0, 0.5));
  	transition: d 0.12s ease-in-out;
  }
  .eye-left,
  .eye-right {
  	filter: drop-shadow(1px 1px 0 rgba(0, 0, 0, 0.5));
  }

  .mood-talking {
  	.eye-left,
  	.eye-right {
  		> g {
  			transition: transform 0.5s ease-in-out;
  		}
  	}
  }

  /* ── Leaves ──────────────────────────────────────────────────────── */
  .leave-right {
  	transform-box: fill-box;
  	transform-origin: 0% 100%;
  }
  .leave-left {
  	transform-box: fill-box;
  	transform-origin: 100% 100%;
  }

  /* idle: slow gentle breathing wave */
  @keyframes idle-right {
  	0%,
  	100% {
  		transform: rotate(0deg);
  	}
  	50% {
  		transform: rotate(-9deg);
  	}
  }
  @keyframes idle-left {
  	0%,
  	100% {
  		transform: rotate(0deg);
  	}
  	50% {
  		transform: rotate(7deg);
  	}
  }
  .mood-idle .leave-right {
  	animation: idle-right 3s ease-in-out infinite;
  }
  .mood-idle .leave-left {
  	animation: idle-left 3s ease-in-out infinite 0.15s;
  }

  /* thinking: wings held raised, minimal drift */
  @keyframes think-right {
  	0%,
  	100% {
  		transform: rotate(-14deg);
  	}
  	50% {
  		transform: rotate(-10deg);
  	}
  }
  @keyframes think-left {
  	0%,
  	100% {
  		transform: rotate(10deg);
  	}
  	50% {
  		transform: rotate(7deg);
  	}
  }
  .mood-thinking .leave-right {
  	animation: think-right 4s ease-in-out infinite;
  }
  .mood-thinking .leave-left {
  	animation: think-left 4s ease-in-out infinite 0.3s;
  }

  /* talking: nearly still — tiny passive counter-sway */
  @keyframes talk-right {
  	0%,
  	100% {
  		transform: rotate(-2deg);
  	}
  	50% {
  		transform: rotate(2deg);
  	}
  }
  @keyframes talk-left {
  	0%,
  	100% {
  		transform: rotate(2deg);
  	}
  	50% {
  		transform: rotate(-2deg);
  	}
  }
  .mood-talking .leave-right {
  	animation: talk-right 0.6s ease-in-out infinite;
  }
  .mood-talking .leave-left {
  	animation: talk-left 0.6s ease-in-out infinite 0.1s;
  }

  /* happy: light casual flap */
  @keyframes happy-right {
  	0%,
  	100% {
  		transform: rotate(0deg);
  	}
  	50% {
  		transform: rotate(-18deg);
  	}
  }
  @keyframes happy-left {
  	0%,
  	100% {
  		transform: rotate(0deg);
  	}
  	50% {
  		transform: rotate(13deg);
  	}
  }
  .mood-happy .leave-right {
  	animation: happy-right 1.4s ease-in-out infinite;
  }
  .mood-happy .leave-left {
  	animation: happy-left 1.4s ease-in-out infinite 0.1s;
  }

  /* moving: vigorous wing flap — full range, fast */
  @keyframes flap-right {
  	0%,
  	100% {
  		transform: rotate(0deg);
  	}
  	40% {
  		transform: rotate(-40deg);
  	}
  }
  @keyframes flap-left {
  	0%,
  	100% {
  		transform: rotate(0deg);
  	}
  	40% {
  		transform: rotate(26deg);
  	}
  }
  .mood-moving .leave-right {
  	animation: flap-right 0.34s ease-in-out infinite;
  }
  .mood-moving .leave-left {
  	animation: flap-left 0.34s ease-in-out infinite 0.04s;
  }

  /* ── Eye blink (on pupil so it doesn't fight cursor translate) ───── */
  @keyframes blink {
  	0%,
  	93%,
  	100% {
  		transform: scaleY(1);
  	}
  	96% {
  		transform: scaleY(0.05);
  	}
  }
  .pupil {
  	transform-box: fill-box;
  	transform-origin: center;
  	animation: blink 4s ease-in-out infinite;
  }
  .eye-left .pupil {
  	animation-delay: 0s;
  }
  .eye-right .pupil {
  	animation-delay: 0.07s;
  }
</style>
