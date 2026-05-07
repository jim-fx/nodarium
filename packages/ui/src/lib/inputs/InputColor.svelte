<script lang="ts">
  interface Props {
    value?: [number, number, number];
    id?: string;
  }

  let {
    value = $bindable([255, 255, 255] as [number, number, number]),
    id
  }: Props = $props();

  let hexValue = $derived(
    `#${value.map((c) => c.toString(16).padStart(2, '0')).join('')}`
  );

  function handleHexInput(e: Event) {
    const target = e.target as HTMLInputElement;
    let val = target.value.replace(/[^0-9a-fA-F]/g, '');
    if (val.length > 6) val = val.slice(0, 6);
    if (val.length === 3) {
      val = val
        .split('')
        .map((c) => c + c)
        .join('');
    }
    if (val.length === 6) {
      value = [
        parseInt(val.slice(0, 2), 16),
        parseInt(val.slice(2, 4), 16),
        parseInt(val.slice(4, 6), 16)
      ] as [number, number, number];
    }
  }
</script>

<div class="flex overflow-hidden rounded-sm border border-outline bg-layer-2 w-min">
  <label
    class="-ml-px w-8 shrink-0 overflow-hidden"
    style={`background-color: ${hexValue}`}
  >
    <input
      type="color"
      bind:value={hexValue}
      {id}
      oninput={handleHexInput}
      class="h-full w-8 cursor-pointer appearance-none p-0"
    />
  </label>
  <div class="flex items-center gap-1 px-2 py-1 border-l border-outline">
    <span class="pointer-events-none text-text opacity-30">#</span>
    <input
      type="text"
      value={hexValue.slice(1)}
      {id}
      oninput={handleHexInput}
      maxlength={6}
      class="w-15 bg-transparent text-text outline-none"
    />
  </div>
</div>

<style>
  input[type="color"] {
    margin-top: -1px;
    margin-right: -1px;
    height: calc(100% + 2px);
    width: calc(100% + 2px);
  }
</style>
