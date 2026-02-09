<script lang="ts">
  import { InputColor } from '$lib';

  const colors = [
    'layer-0',
    'layer-1',
    'layer-2',
    'layer-3',
    'active',
    'selected',
    'outline',
    'connection',
    'text'
  ];

  type CustomColors = {
    text: [number, number, number];
    outline: [number, number, number];
    'layer-0': [number, number, number];
    'layer-1': [number, number, number];
    'layer-2': [number, number, number];
    'layer-3': [number, number, number];
    active: [number, number, number];
    selected: [number, number, number];
    connection: [number, number, number];
  };

  type CustomColorKey = keyof CustomColors;

  let customColors = $state<CustomColors>({
    text: [205, 214, 244],
    outline: [62, 62, 79],
    'layer-0': [6, 6, 27],
    'layer-1': [23, 23, 46],
    'layer-2': [49, 50, 68],
    'layer-3': [168, 170, 200],
    active: [0, 0, 0],
    selected: [38, 139, 210],
    connection: [131, 148, 150]
  });

  const themeCss = $derived.by(() => {
    return `<style>html.theme-custom{
  ${
      Object.keys(customColors)
        .map((v) => {
          return `--color-${v}: rgb(${customColors[v as CustomColorKey].join(',')});`;
        })
        .join('\n')
    }</style>`;
  });
</script>

<svelte:head>
  <!-- eslint-disable-next-line svelte/no-at-html-tags -->
  {@html themeCss}
</svelte:head>

<table>
  <thead>
    <tr>
      <th>Color</th>
      <th>Name</th>
      <th>Custom</th>
    </tr>
  </thead>
  <tbody>
    {#each colors as color (color)}
      <tr>
        <td>
          <div class="w-6 h-6 mr-2 my-1 rounded-sm outline-1 bg-{color}"></div>
        </td>
        <td>{color}</td>
        <td>
          <InputColor bind:value={customColors[color as CustomColorKey]} />
        </td>
      </tr>
    {/each}
  </tbody>
</table>

<style>
  table {
    border-spacing: 5px;
    border-collapse: separate;
    text-align: left;
    margin-left: 5px;
  }
</style>
