<script lang="ts">
  import { localState } from '#lib/helpers/localState.svelte.ts';
  import { getGridId, hasGridId, setGridRow } from './context';

  const gridId = (hasGridId() && getGridId()) || 'grid-0';
  let sizes = localState<string[]>(gridId, []);

  const { children } = $props();

  let registerIndex = 0;
  setGridRow({
    registerCell() {
      const index = registerIndex;
      registerIndex++;
      if (registerIndex > sizes.value.length) {
        sizes.value = [...sizes.value, '1fr'];
      }
      return index;
    },
    sizes
  });

  const cols = $derived(
    sizes.value.map((size, i) => `${i > 0 ? '1px ' : ''}` + size).join(' ')
  );
</script>

<div class="wrapper" style={`grid-template-columns: ${cols};`}>
  {@render children()}
</div>

<style>
  .wrapper {
    display: grid;
    height: 100%;
  }
</style>
