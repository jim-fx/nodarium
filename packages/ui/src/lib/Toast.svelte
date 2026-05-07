<script lang="ts">
  import { fly, slide } from 'svelte/transition';
  import { toasts } from './toast.svelte';

  const typeClasses: Record<string, string> = {
    success: 'border-l-green-500',
    error: 'border-l-red-500',
    info: 'border-l-active'
  };
</script>

<div
  class="fixed bottom-4 right-4 flex flex-col items-end gap-2 z-9999 pointer-events-none"
  role="status"
  aria-live="polite"
  aria-atomic="false"
>
  {#each toasts.value as item (item.id)}
    <div
      in:slide={{ duration: 250 }}
      out:fly={{ x: 100, duration: 250 }}
      class="
        bg-layer-2 text-text border border-outline rounded
        px-3.5 py-2 text-sm min-w-45 max-w-xs w-fit
        border-l-3 {typeClasses[item.type] ?? 'border-l-outline'}
      "
    >
      {item.message}
    </div>
  {/each}
</div>
