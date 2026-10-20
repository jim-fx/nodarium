<script lang="ts">
  import { errorLog, type ErrorLogEntry } from '$lib/runtime/error-log.svelte';
  import { Button } from '@nodarium/ui';

  type Props = {
    onfocusnode?: (nodeId: number) => void;
  };

  const { onfocusnode }: Props = $props();

  function formatTime(time: number) {
    return new Date(time).toLocaleTimeString(undefined, {
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit'
    });
  }

  function reason(entry: ErrorLogEntry) {
    // the message starts with the node title, which the entry already shows
    const prefix = `${entry.nodeTitle} failed: `;
    return entry.nodeTitle && entry.message.startsWith(prefix)
      ? entry.message.slice(prefix.length)
      : entry.message;
  }
</script>

<div class="flex flex-col gap-2 p-2">
  {#if errorLog.entries.length}
    <div class="flex justify-end">
      <Button size="sm" onclick={() => errorLog.clear()}>Clear</Button>
    </div>
    <ul class="flex flex-col gap-2">
      {#each errorLog.entries as entry (entry.id)}
        <li class="flex flex-col gap-1 border-l-2 border-l-error bg-layer-2 px-2 py-1.5 text-sm">
          <div class="flex items-center gap-2 text-xs opacity-60">
            <span>{formatTime(entry.time)}</span>
            {#if entry.nodeId !== undefined}
              <button
                class="ml-auto cursor-pointer hover:underline"
                onclick={() => onfocusnode?.(entry.nodeId!)}
              >
                {entry.nodeTitle} #{entry.nodeId}
              </button>
            {/if}
          </div>
          <span class="break-words">{reason(entry)}</span>
          {#if entry.location}
            <span class="font-mono text-xs break-all opacity-50">{entry.location}</span>
          {/if}
        </li>
      {/each}
    </ul>
  {:else}
    <span class="text-sm opacity-50">No errors</span>
  {/if}
</div>
