<script lang="ts">
  import { toasts } from './toast.svelte';

  const typeClasses: Record<string, string> = {
    success: 'border-l-green-500',
    error: 'border-l-red-500',
    info: 'border-l-active'
  };
</script>

<div
  class="fixed bottom-4 right-4 flex flex-col gap-2 z-[9999] pointer-events-none"
  role="status"
  aria-live="polite"
  aria-atomic="false"
>
  {#each toasts.value as item (item.id)}
    <div
      class="
        bg-layer-2 text-text border border-outline rounded
        px-3.5 py-2 text-sm min-w-[180px] max-w-xs
        border-l-3 {typeClasses[item.type] ?? 'border-l-outline'}
        animate-[slide-in_0.18s_ease]
      "
    >
      {item.message}
    </div>
  {/each}
</div>

<style>
  @keyframes slide-in {
    from { opacity: 0; transform: translateX(12px); }
    to   { opacity: 1; transform: translateX(0); }
  }
</style>
