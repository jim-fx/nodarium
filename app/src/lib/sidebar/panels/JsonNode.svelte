<script module>
  const cache = new Map<string, Record<string, boolean>>();

  function getStore(root: string): Record<string, boolean> {
    if (!cache.has(root)) {
      try {
        const raw = localStorage.getItem(`json_viewer:${root}`);
        cache.set(root, raw ? JSON.parse(raw) : {});
      } catch {
        cache.set(root, {});
      }
    }
    return cache.get(root)!;
  }

  function readOpen(path: string, fallback: boolean): boolean {
    const root = path.split('/')[0];
    const store = getStore(root);
    return path in store ? store[path] : fallback;
  }

  function writeOpen(path: string, value: boolean) {
    const root = path.split('/')[0];
    const store = getStore(root);
    store[path] = value;
    try {
      localStorage.setItem(`json_viewer:${root}`, JSON.stringify(store));
    } catch { /* quota exceeded etc */ }
  }
</script>

<script lang="ts">
  import { browser } from '$app/environment';
  import JsonNode from './JsonNode.svelte';

  let {
    value,
    key,
    depth = 0,
    path = ''
  }: { value: unknown; key?: string; depth?: number; path?: string } = $props();

  const defaultOpen = $derived(depth < 4);
  let open = $derived(browser && path ? readOpen(path, defaultOpen) : defaultOpen);
  let flashing = $state(false);

  const isArr = $derived(Array.isArray(value));
  const isExpandable = $derived(value !== null && typeof value === 'object');
  const open_bracket = $derived(isArr ? '[' : '{');
  const close_bracket = $derived(isArr ? ']' : '}');
  const items = $derived.by(() => {
    if (Array.isArray(value)) {
      return (value as unknown[]).map((v, i) => [String(i), v] as [string, unknown]);
    }
    if (value !== null && typeof value === 'object') {
      return Object.entries(value as Record<string, unknown>).filter(
        ([, v]) => v !== undefined
      );
    }
    return [] as [string, unknown][];
  });

  function toggle(next: boolean) {
    open = next;
    if (browser && path) writeOpen(path, next);
  }

  let prevJson = '';
  let flashTimeout: ReturnType<typeof setTimeout> | null = null;

  $effect(() => {
    const json = JSON.stringify(value);
    if (prevJson && json !== prevJson) {
      if (flashTimeout) clearTimeout(flashTimeout);
      flashing = true;
      flashTimeout = setTimeout(() => {
        flashing = false;
        flashTimeout = null;
      }, 500);
    }
    prevJson = json;
  });
</script>

<span
  class="font-mono text-xs leading-[1.6] rounded transition-[background-color] duration-500"
  class:bg-white={flashing}
>
  {#if key !== undefined}
    <span class="text-neutral-300">{key}</span><span class="text-neutral-600">: </span>
  {/if}

  {#if isExpandable}
    {#if items.length === 0}
      <span class="text-neutral-500">{open_bracket}{close_bracket}</span>
    {:else if open}
      {#if depth > 0}
        <button class="w-3 text-neutral-500 hover:text-neutral-200" onclick={() => toggle(false)}>
          ▼
        </button>
      {/if}
      <span class="text-neutral-500">{open_bracket}</span>
      <div class="pl-4 border-l border-neutral-700/60">
        {#each items as [k, v], i (k)}
          <div>
            <JsonNode
              value={v}
              key={k}
              depth={depth + 1}
              path={path ? `${path}/${k}` : k}
            />{#if i < items.length - 1}<span class="text-neutral-700">,</span>{/if}
          </div>
        {/each}
      </div>
      <span class="text-neutral-500">{close_bracket}</span>
    {:else}
      <button
        class="inline text-neutral-500 hover:text-neutral-200"
        onclick={() => toggle(true)}
      >
        <span class="w-3 inline-block">▶</span>
        {open_bracket}<span class="text-neutral-600 mx-1">{items.length}</span>{close_bracket}
      </button>
    {/if}
  {:else if value === null}
    <span class="text-neutral-500!">null</span>
  {:else if typeof value === 'boolean'}
    <span class="text-amber-400!">{value}</span>
  {:else if typeof value === 'number'}
    <span class="text-sky-400!">{value}</span>
  {:else if typeof value === 'string'}
    <span class="text-emerald-400!">"{value}"</span>
  {:else}
    <span class="text-neutral-400">{String(value)}</span>
  {/if}
</span>
