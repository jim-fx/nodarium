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

<div class="wrapper" style:--columns={cols}>
  {@render children()}
</div>

<style>
  .wrapper {
    display: grid;
    grid-template-columns: var(--columns);
    height: 100%;
  }

  /* a phone held upright, two cells next to each other would be too narrow to use */
  @media (max-width: 599px) {
    .wrapper {
      grid-template-columns: minmax(0, 1fr);
      grid-template-rows: minmax(0, 3fr) 1px minmax(0, 2fr);
    }
  }
</style>
