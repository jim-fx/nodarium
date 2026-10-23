<script lang="ts">
  import { colors } from '#lib/graph-interface/graph/colors.svelte.ts';
  import { T } from '@threlte/core';
  import { Text } from '@threlte/extras';
  import { type Box3, type EulerTuple, Vector3 } from 'three';
  import { appSettings } from '../../settings/app-settings.svelte';
  import { viewScale } from './view-scale.svelte';

  const { bounds }: { bounds?: Box3 } = $props();

  // colors are updated in place on theme changes, so the theme is the dependency
  const color = $derived.by(() => {
    void appSettings.value.theme;
    return colors.text.getHex();
  });
  const outlineColor = $derived.by(() => {
    void appSettings.value.theme;
    return colors['layer-0'].getHex();
  });

  // all three arrows start at the same bottom corner of the bounding box
  const origin = $derived(
    bounds && new Vector3(bounds.min.x, bounds.min.y, bounds.max.z)
  );

  const axes = $derived.by(() => {
    if (!bounds || !origin) return [];
    const size = bounds.getSize(new Vector3());
    // floor labels hover slightly, so they don't z-fight with the grid
    const lift = new Vector3(0, 0.01 * viewScale.value, 0);
    const along = (dir: Vector3, length: number) => origin.clone().addScaledVector(dir, length / 2);
    return [
      {
        label: 'Width',
        dir: new Vector3(1, 0, 0),
        length: size.x,
        // flat on the floor in front of the arrow, running along x
        textPosition: along(new Vector3(1, 0, 0), size.x).add(lift),
        rotation: [-Math.PI / 2, Math.PI, 0, 'YXZ'] as EulerTuple,
        anchorX: 'center' as const,
        anchorY: 'bottom' as const
      },
      {
        label: 'Height',
        dir: new Vector3(0, 1, 0),
        length: size.y,
        // upright, next to the arrow
        textPosition: along(new Vector3(0, 1, 0), size.y),
        rotation: [0, Math.PI / 2, Math.PI / 2, 'XYZ'] as EulerTuple,
        anchorX: 'center' as const,
        anchorY: 'bottom' as const
      },
      {
        label: 'Depth',
        dir: new Vector3(0, 0, -1),
        length: size.z,
        // flat on the floor beside the arrow, running along z
        textPosition: along(new Vector3(0, 0, -1), size.z).add(lift),
        rotation: [-Math.PI / 2, Math.PI / 2, 0, 'YXZ'] as EulerTuple,
        anchorX: 'center' as const,
        anchorY: 'bottom' as const
      }
    ].filter((a) => a.length > 0);
  });

  /** Picks the unit that keeps the number readable: below 1m is cm, below 1cm is mm */
  function format(meters: number) {
    if (meters >= 1) return `${+meters.toFixed(2)} m`;
    if (meters >= 0.01) return `${+(meters * 100).toFixed(2)} cm`;
    return `${+(meters * 1000).toFixed(2)} mm`;
  }
</script>

{#if bounds && origin}
  {#each axes as axis (axis.label)}
    <T.ArrowHelper
      args={[
        axis.dir,
        origin,
        axis.length,
        color,
        Math.min(0.3 * viewScale.value, axis.length / 3),
        0.15 * viewScale.value
      ]}
    />
    <T.Group position={axis.textPosition.toArray()}>
      <T.Group rotation={axis.rotation}>
        <Text
          text={format(axis.length)}
          fontSize={0.25 * viewScale.value}
          font="/fonts/FiraCode-Light.ttf"
          color={color}
          anchorX={axis.anchorX}
          anchorY={axis.anchorY}
          position.y={0.05 * viewScale.value}
          outlineWidth={0.02 * viewScale.value}
          {outlineColor}
        />
      </T.Group>
    </T.Group>
  {/each}
{/if}
