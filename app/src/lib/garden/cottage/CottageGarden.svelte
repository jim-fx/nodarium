<script lang="ts">
  import { T, useTask } from '@threlte/core';
  import Dust from '../Dust.svelte';
  import GardenEnvironment from '../GardenEnvironment.svelte';
  import Butterflies from './Butterflies.svelte';
  import { COLORS } from './config';
  import Flora from './Flora.svelte';
  import Hedges from './Hedges.svelte';
  import Path from './Path.svelte';
  import RaisedBed from './RaisedBed.svelte';
  import { windUniforms } from './wind';

  type Props = {
    plants: { x: number; z: number }[];
    onground: () => void;
  };
  const { plants, onground }: Props = $props();

  useTask((delta) => {
    windUniforms.windTime.value += delta;
  });
</script>

<!-- the lush hdri stays visible behind the hedges, far out of focus -->
<GardenEnvironment
  style="cottage"
  intensity={0.45}
  background={{ blurriness: 0.35, intensity: 0.85, tilt: -0.5 }}
/>

<!-- a soft summer haze in the distance -->
<T.Fog attach="fog" args={[COLORS.haze, 30, 90]} />

<T.HemisphereLight args={[COLORS.sky, '#4d5e2c', 0.45]} />
<!-- low afternoon sun from the side, it rakes over the borders and the bed -->
<T.DirectionalLight
  color={COLORS.sun}
  intensity={5.5}
  position={[-17, 11, -6]}
  castShadow
  shadow.mapSize={[4096, 4096]}
  shadow.camera.left={-24}
  shadow.camera.right={24}
  shadow.camera.top={20}
  shadow.camera.bottom={-20}
  shadow.camera.far={70}
  shadow.bias={-0.0003}
  shadow.normalBias={0.03}
  shadow.radius={3}
/>
<T.DirectionalLight color="#fff6e8" intensity={0.5} position={[8, 6, 14]} />

<RaisedBed {onground} />
<Path />
<Hedges />
<Flora {plants} />
<Butterflies />

<Dust color={COLORS.pollen} count={420} size={4.5} opacity={0.55} spread={14} />
