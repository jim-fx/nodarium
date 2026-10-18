<script lang="ts">
  import { T } from '@threlte/core';
  import type { Texture } from 'three';
  import Dust from '../Dust.svelte';
  import GardenEnvironment from '../GardenEnvironment.svelte';
  import type { Island } from '../layout';
  import type { Surface } from '../textures';
  import Bed from './Bed.svelte';
  import { COLORS, ROCKS } from './config';
  import Pond from './Pond.svelte';
  import Rock from './Rock.svelte';
  import Surroundings from './Surroundings.svelte';

  type Props = { islands: Island[]; stone: Surface; mossMap: Texture; onground: () => void };
  const { islands, stone, mossMap, onground }: Props = $props();
</script>

<!-- the hdri only lights the scene softly, the background stays dark -->
<GardenEnvironment style="zen" intensity={0.35} background={COLORS.background} />

<T.Fog attach="fog" args={[COLORS.background, 24, 44]} />

<T.HemisphereLight args={['#c9ccd2', '#1a1a1a', 0.7]} />
<T.DirectionalLight
  color={COLORS.light}
  intensity={3}
  position={[-9, 9, -6]}
  castShadow
  shadow.mapSize={[2048, 2048]}
  shadow.camera.left={-12}
  shadow.camera.right={12}
  shadow.camera.top={9}
  shadow.camera.bottom={-9}
  shadow.camera.far={40}
  shadow.bias={-0.0004}
  shadow.normalBias={0.02}
  shadow.radius={4}
/>
<T.DirectionalLight color="#dfe4ea" intensity={0.4} position={[6, 5, 10]} />

<Bed {islands} {onground} />

<Pond {stone} />

{#each ROCKS as rock (rock.seed)}
  <Rock {rock} {stone} {mossMap} />
{/each}

<Surroundings />

<Dust color={COLORS.light} />
