<script module lang="ts">
  import { SvelteMap } from 'svelte/reactivity';
  const cache = new SvelteMap<string, Record<string, boolean>>();

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
  import JsonViewer from './JsonViewer.svelte';

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
    if (isArr) {
      return (value as unknown[]).map((v, i) => [String(i), v] as [string, unknown]);
    }
    if (value !== null && typeof value === 'object') {
      return Object.entries(value as Record<string, unknown>).filter(
        ([, v]) => v !== undefined
      );
    }
    return [] as [string, unknown][];
  });
  const showKeys = $derived(!isArr || typeof items[0]?.[1] === 'object');

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
  class:bg-layer-3={flashing}
>
  {#if key !== undefined}
    <button
      class="text-text hover:bg-layer-3 cursor-pointer"
      title="Copy value"
      onclick={() => navigator.clipboard.writeText(JSON.stringify({ [key]: value }, null, 2))}
    >
      {key}
    </button><span class="text-text/40">: </span>
  {/if}

  {#if isExpandable}
    {#if items.length === 0}
      <span class="text-text/50">{open_bracket}{close_bracket}</span>
    {:else if open}
      {#if depth > 0}
        <button class="w-3 text-text/50 hover:text-text" onclick={() => toggle(false)}>
          ▼
        </button>
      {/if}
      <span class="text-text/50">{open_bracket}</span>
      <div class="pl-4 border-l border-outline">
        {#each items as [k, v], i (k)}
          <div>
            <JsonViewer
              value={v}
              key={showKeys ? k : undefined}
              depth={depth + 1}
              path={path ? `${path}/${k}` : k}
            />{#if i < items.length - 1}<span class="text-text/20">,</span>{/if}
          </div>
        {/each}
      </div>
      <span class="text-text/50">{close_bracket}</span>
    {:else}
      <button
        class="inline text-text/50 hover:text-text"
        onclick={() => toggle(true)}
      >
        <span class="w-3 inline-block">▶</span>
        {open_bracket}<span class="text-text/40 mx-1">{items.length}</span>{close_bracket}
      </button>
    {/if}
  {:else if value === null}
    <span class="text-emerald-500!">null</span>
  {:else if typeof value === 'boolean'}
    <span class="text-blue-500!">{value}</span>
  {:else if typeof value === 'number'}
    <span class="text-orange-400!">{value}</span>
  {:else if typeof value === 'string'}
    <span class="text-emerald-500!">"{value}"</span>
  {:else}
    <span class="text-text/70">{String(value)}</span>
  {/if}
</span>
