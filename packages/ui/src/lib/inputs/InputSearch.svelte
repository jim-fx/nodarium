<script lang="ts">
  type SelectOption = string | { value: number; label: string };

  interface Props {
    options?: SelectOption[];
    value?: number;
    id?: string;
    placeholder?: string;
  }

  let {
    options = [],
    value = $bindable(0),
    id = '',
    placeholder = 'Search…'
  }: Props = $props();

  const normalized = $derived(
    options.map((opt, i) => typeof opt === 'string' ? { value: i, label: opt } : opt)
  );

  const selected = $derived(normalized.find((o) => o.value === value));

  let query = $state('');
  let open = $state(false);
  let container: HTMLDivElement;

  const filtered = $derived(
    query === ''
      ? normalized
      : normalized.filter((o) => o.label.toLowerCase().includes(query.toLowerCase()))
  );

  function select(val: number) {
    value = val;
    query = '';
    open = false;
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      open = false;
      query = '';
    }
    if (e.key === 'ArrowDown' && filtered.length) {
      const idx = filtered.findIndex((o) => o.value === value);
      value = filtered[(idx + 1) % filtered.length].value;
    }
    if (e.key === 'ArrowUp' && filtered.length) {
      const idx = filtered.findIndex((o) => o.value === value);
      value = filtered[(idx - 1 + filtered.length) % filtered.length].value;
    }
    if (e.key === 'Enter' && filtered.length) {
      const match = filtered.find((o) => o.value === value) ?? filtered[0];
      select(match.value);
    }
  }

  function handleBlur(e: FocusEvent) {
    if (!container.contains(e.relatedTarget as Node)) {
      open = false;
      query = '';
    }
  }
</script>

<div class="relative w-full" bind:this={container} onblur={handleBlur}>
  <input
    {id}
    type="text"
    class:rounded-b-none!={open}
    class="w-full bg-layer-2 text-text outline outline-outline px-3 py-2 rounded-md border-none font-(--font-family) text-sm box-border focus:outline-2 focus:outline-active"
    placeholder={open ? placeholder : (selected?.label ?? placeholder)}
    bind:value={query}
    onfocus={() => (open = true)}
    onkeydown={handleKeydown}
    autocomplete="off"
  />
  {#if open}
    <div
      class="absolute w-[calc(100%+2px)] -ml-px top-[calc(100%+2px)] left-0 right-0 bg-layer-1 border border-outline rounded-b-md max-h-50 overflow-y-auto z-100"
      role="listbox"
    >
      {#each filtered as opt (opt.value)}
        <div
          class="px-3 py-2 text-sm text-text cursor-pointer font-(--font-family) {opt.value === value ? 'bg-layer-2' : 'hover:bg-layer-2'}"
          role="option"
          aria-selected={opt.value === value}
          tabindex="-1"
          onmousedown={() => select(opt.value)}
        >
          {opt.label}
        </div>
      {:else}
        <div class="px-3 py-2 text-xs text-text opacity-45 italic">No results</div>
      {/each}
    </div>
  {/if}
</div>
