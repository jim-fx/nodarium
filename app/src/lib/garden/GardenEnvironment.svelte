<script lang="ts">
  import { useThrelte } from '@threlte/core';
  import { onMount } from 'svelte';
  import { Color } from 'three';
  import type { GardenStyle } from './config';
  import { loadEnvironment } from './textures';

  type Props = {
    style: GardenStyle;
    /** how strongly the hdri lights the scene */
    intensity: number;
    /**
     * a plain color, or the hdri itself blurred out of focus,
     * tilted down so more of its tree tops are behind the garden
     */
    background: string | { blurriness: number; intensity: number; tilt: number };
  };
  const { style, intensity, background }: Props = $props();

  const { scene } = useThrelte();

  onMount(() => {
    let disposed = false;
    scene.environmentIntensity = intensity;
    if (typeof background === 'string') {
      scene.background = new Color(background);
    }

    loadEnvironment(style).then((texture) => {
      if (disposed) return texture.dispose();
      scene.environment = texture;
      if (typeof background !== 'string') {
        scene.background = texture;
        scene.backgroundBlurriness = background.blurriness;
        scene.backgroundIntensity = background.intensity;
        scene.backgroundRotation.set(background.tilt, 0, 0);
      }
    });

    return () => {
      disposed = true;
      scene.environment?.dispose();
      scene.environment = null;
      scene.background = null;
      scene.backgroundBlurriness = 0;
      scene.backgroundIntensity = 1;
      scene.backgroundRotation.set(0, 0, 0);
    };
  });
</script>
