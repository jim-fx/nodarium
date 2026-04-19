<script lang="ts">
  import type { PlantyHook } from '../types.js';

  interface Props {
    selector?: string;
    hookName?: string;
    hooks?: Record<string, PlantyHook>;
  }

  let { selector, hookName, hooks = {} }: Props = $props();

  let rect = $state<{ top: number; left: number; width: number; height: number } | null>(null);

  $effect(() => {
    let el: Element | null = null;
    let ro: ResizeObserver | null = null;
    let mo: MutationObserver | null = null;

    function resolveEl(): Element | null {
      if (selector) return document.querySelector(selector);
      if (hookName && hooks[hookName]) {
        const result = hooks[hookName]();
        if (result instanceof Element) return result;
      }
      return null;
    }

    function updateRect() {
      if (!el) {
        rect = null;
        return;
      }
      const raw = el.getBoundingClientRect();
      const vw = window.innerWidth;
      const vh = window.innerHeight;
      const p = 4;
      const top = Math.max(p, raw.top - p);
      const left = Math.max(p, raw.left - p);
      const right = Math.min(vw - p, raw.right + p);
      const bottom = Math.min(vh - p, raw.bottom + p);
      if (right <= left || bottom <= top) {
        rect = null;
        return;
      }
      rect = { top, left, width: right - left, height: bottom - top };
    }

    function attachEl(newEl: Element | null) {
      if (newEl === el) return;
      ro?.disconnect();
      el = newEl;
      if (!el) {
        rect = null;
        return;
      }
      updateRect();
      ro = new ResizeObserver(updateRect);
      ro.observe(el);
    }

    attachEl(resolveEl());

    window.addEventListener('scroll', updateRect, { passive: true, capture: true });
    window.addEventListener('resize', updateRect, { passive: true });

    // For hook-based highlights, watch the DOM so we catch dynamically added elements
    if (hookName) {
      mo = new MutationObserver(() => attachEl(resolveEl()));
      mo.observe(document.body, { childList: true, subtree: true });
    }

    return () => {
      ro?.disconnect();
      mo?.disconnect();
      window.removeEventListener('scroll', updateRect, true);
      window.removeEventListener('resize', updateRect);
    };
  });
</script>

{#if rect}
  <div
    class="highlight pointer-events-none fixed z-99999 rounded-md"
    style:top="{rect.top}px"
    style:left="{rect.left}px"
    style:width="{rect.width}px"
    style:height="{rect.height}px"
  >
  </div>
{/if}

<style>
  @keyframes pulse {
  	0%,
  	100% {
  		box-shadow:
  			0 0 0 9999px rgba(0, 0, 0, 0.45),
  			0 0 0 2px rgba(255, 255, 255, 0.9),
  			0 0 16px rgba(255, 255, 255, 0.3);
  	}
  	50% {
  		box-shadow:
  			0 0 0 9999px rgba(0, 0, 0, 0.45),
  			0 0 0 2px rgba(255, 255, 255, 1),
  			0 0 28px rgba(255, 255, 255, 0.6);
  	}
  }
  .highlight {
  	animation: pulse 1.8s ease-in-out infinite;
  }
</style>
