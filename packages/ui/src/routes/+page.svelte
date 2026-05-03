<script lang="ts">
  import '$lib/app.css';
  import {
    Details,
    InputCheckbox,
    InputColor,
    InputNumber,
    InputSelect,
    InputShape,
    InputVec3,
    JsonViewer,
    ShortCut
  } from '$lib';
  import Section from './Section.svelte';
  import Theme from './Theme.svelte';
  import ThemeSelector from './ThemeSelector.svelte';

  let intValue = $state(0);
  let floatValue = $state(0.2);
  let float3Value = $state(1);
  let vecValue = $state([0.2, 0.3, 0.4]);
  const options = ['strawberry', 'raspberry', 'chickpeas'];
  let selectValue = $state(0);
  const d = $derived(options[selectValue]);
  let checked = $state(false);
  let colorValue = $state<[number, number, number]>([59, 130, 246]);
  let mirrorShape = $state(true);
  let detailsOpen = $state(false);
  let jsonValue = $state({
    id: 1,
    nodes: [{ id: 0, type: 'max/test/node', position: [0, 0] }, {
      id: 1,
      type: 'max/test/other',
      position: [100, 50]
    }],
    edges: [[0, 0, 1, 'input']],
    groups: [],
    settings: { seed: 42, enabled: true }
  });

  function randomlyUpdateJson() {
    const rand = Math.floor(Math.random() * 5);
    if (rand === 0) {
      jsonValue.nodes[0].position[0] += 1;
    } else if (rand === 1) {
      jsonValue.nodes[0].position[1] += 1;
    } else if (rand === 2) {
      jsonValue.settings.seed += 1;
    } else if (rand === 3) {
      jsonValue.settings.enabled = !jsonValue.settings.enabled;
    } else if (rand === 4) {
      jsonValue.id += Math.floor(Math.random() * 10 - 5);
    }
  }

  let points = $state([]);
  let theme = $state('dark');
</script>

<main class="flex flex-col gap-8 py-8">
  <div class="flex gap-4">
    <h1 class="text-4xl">@nodarium/ui</h1>
    <ThemeSelector bind:theme />
  </div>

  <Section title="InputNumber">
    <Theme />
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
    <i>Select with simple values</i>
    <InputSelect bind:value={selectValue} {options} />
  </Section>

  <Section title="Checkbox" value={checked}>
    <InputCheckbox bind:value={checked} />
  </Section>

  <Section title="Color" value={colorValue}>
    <InputColor bind:value={colorValue} />
  </Section>

  <Section title="Shape">
    {#snippet header()}
      <label class="flex gap-2">
        <InputCheckbox bind:value={mirrorShape} />
        <p>mirror</p>
      </label>
      <p class="max-w-full overflow-hidden">{JSON.stringify(points)}</p>
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

  <Section title="JsonViewer">
    {#snippet header()}
      <button
        onclick={() => randomlyUpdateJson()}
        class="-mt-1 bg-layer-2 p-1 px-2 rounded-sm cursor-pointer"
      >
        update
      </button>
    {/snippet}
    <div class="w-64 bg-layer-1 p-2 rounded">
      <JsonViewer
        value={jsonValue}
        path="demo"
      />
    </div>
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
