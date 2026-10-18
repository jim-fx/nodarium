<script lang="ts">
  import type { Graph } from '@nodarium/types';
  import { interactivity } from '@threlte/extras';
  import { Vector3 } from 'three';
  import { type GardenStyle, PLANT_ISLAND_RADIUS } from './config';
  import { COTTAGE } from './cottage/config';
  import CottageGarden from './cottage/CottageGarden.svelte';
  import DepthOfField from './DepthOfField.svelte';
  import GardenCamera from './GardenCamera.svelte';
  import { layoutPlants } from './layout';
  import Plant from './Plant.svelte';
  import { loadGeneratedTexture, loadSurface } from './textures';
  import { OBSTACLES, ZEN } from './zen/config';
  import ZenGarden from './zen/ZenGarden.svelte';

  type Props = {
    style: GardenStyle;
    projects: Graph[];
    results: Map<number, Int32Array | null>;
  };
  const { style, projects, results }: Props = $props();

  interactivity();

  const settings = $derived(style === 'zen' ? ZEN : COTTAGE);

  const stone = loadSurface('rock', 1.2);
  const mossMap = loadGeneratedTexture('moss', 1.2);

  const positions = $derived(
    layoutPlants(
      projects.map((p) => p.id),
      style === 'zen' ? OBSTACLES : []
    )
  );
  const islands = $derived([
    // the pond has its own shore, only the rocks and plants get rings
    ...OBSTACLES.slice(1),
    ...[...positions.values()].map(({ x, z }) => ({ x, z, radius: PLANT_ISLAND_RADIUS }))
  ]);

  let focused = $state<number>();
  let focus = $state<{ x: number; y: number; z: number; height: number }>();
  /** where the camera looks at, that is what stays sharp */
  const cameraTarget = new Vector3();

  function unfocus() {
    focused = undefined;
    focus = undefined;
  }
</script>

<svelte:window onkeydown={(event) => event.key === 'Escape' && unfocus()} />

<GardenCamera {focus} target={cameraTarget} fog={settings.fog} />
<DepthOfField
  focus={cameraTarget}
  aperture={focus ? settings.aperture.focus : settings.aperture.overview}
  maxBlur={settings.maxBlur}
  bloom={settings.bloom}
  vignette={settings.vignette}
/>

{#if style === 'zen'}
  <ZenGarden {islands} {stone} {mossMap} onground={unfocus} />
{:else}
  <CottageGarden plants={[...positions.values()]} onground={unfocus} />
{/if}

{#each projects as project (project.id)}
  {@const position = positions.get(project.id)}
  {#if position}
    <Plant
      id={project.id}
      title={project.meta?.title || 'Untitled'}
      {position}
      result={results.get(project.id)}
      {stone}
      color={settings.plant}
      cushion={settings.cushion ? mossMap : undefined}
      focused={focused === project.id}
      onfocus={(target) => {
        if (focused === project.id) {
          unfocus();
        } else {
          focused = project.id;
          focus = target;
        }
      }}
    />
  {/if}
{/each}
