<script lang="ts">
  import '$lib/app.css';
  import {
    Details,
    InputCheckbox,
    InputNumber,
    InputSelect,
    InputShape,
    InputVec3,
    ShortCut
  } from '$lib';
  import Section from './Section.svelte';

  let intValue = $state(0);
  let floatValue = $state(0.2);
  let float3Value = $state(1);
  let vecValue = $state([0.2, 0.3, 0.4]);
  const options = ['strawberry', 'raspberry', 'chickpeas'];
  let selectValue = $state(0);
  const d = $derived(options[selectValue]);
  let checked = $state(false);
  let mirrorShape = $state(true);
  let detailsOpen = $state(false);

  let points = $state([]);

  const themes = [
    'dark',
    'light',
    'solarized',
    'catppuccin',
    'high-contrast',
    'nord',
    'dracula'
  ];
  let themeIndex = $state(0);
  $effect(() => {
    const classList = document.documentElement.classList;
    for (const c of classList) {
      if (c.startsWith('theme-')) document.documentElement.classList.remove(c);
    }
    document.documentElement.classList.add(`theme-${themes[themeIndex]}`);
  });

  const colors = [
    'layer-0',
    'layer-1',
    'layer-2',
    'layer-3',
    'active',
    'selected',
    'outline',
    'connection',
    'edge',
    'text'
  ];
</script>

<main class="flex flex-col gap-8 py-8">
  <div class="flex gap-4">
    <h1 class="text-4xl">@nodarium/ui</h1>
    <InputSelect bind:value={themeIndex} options={themes}></InputSelect>
  </div>

  <Section title="Colors">
    <table>
      <tbody>
        {#each colors as color (color)}
          <tr>
            <td>
              <div class="w-6 h-6 mr-2 my-1 rounded-sm outline-1 bg-{color}"></div>
            </td>
            <td>{color}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  </Section>

  <Section title="InputNumber">
    <div class="flex flex-col gap-2">
      <p>value={floatValue}</p>
      <InputNumber bind:value={floatValue} />
      <p>min=0 max=10 step=1 value={intValue}</p>
      <InputNumber bind:value={intValue} min={0} max={10} step={1} />
      <p>min=0 max=3 step=0.01 value={float3Value}</p>
      <InputNumber bind:value={float3Value} step={0.01} max={3} />
    </div>
  </Section>

  <Section title="Vec3" value={JSON.stringify(vecValue)}>
    <InputVec3 bind:value={vecValue} />
  </Section>

  <Section title="Select" value={d}>
    <InputSelect bind:value={selectValue} {options} />
  </Section>

  <Section title="Checkbox" value={checked}>
    <InputCheckbox bind:value={checked} />
  </Section>

  <Section title="Shape">
    {#snippet header()}
      <label class="flex gap-2">
        <InputCheckbox bind:value={mirrorShape} />
        <p>mirror</p>
      </label>
      <p>{JSON.stringify(points)}</p>
    {/snippet}
    <div style:width="300px">
      <InputShape bind:value={points} mirror={mirrorShape} />
    </div>
  </Section>

  <Section title="Details" value={detailsOpen}>
    <Details title="More Information" bind:open={detailsOpen}>
      <p>Here is some more information that was previously hidden.</p>
    </Details>
  </Section>

  <Section title="Shortcut">
    <div class="flex gap-4">
      <ShortCut ctrl key="S" />
      <ShortCut alt key="F4" />
      <ShortCut alt ctrl key="delete" />
    </div>
  </Section>
</main>

<style>
  main {
    max-width: 800px;
    margin: 0 auto;
  }
</style>
