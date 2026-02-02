<script lang="ts">
  import type { Box } from '@nodarium/types';
  import { T } from '@threlte/core';
  import { MeshLineGeometry, MeshLineMaterial } from '@threlte/extras';
  import { Color, Vector3 } from 'three';
  import { lines, points, rects } from './store.js';

  type Line = {
    points: Vector3[];
    color?: Color;
  };

  function getEachKey(value: Vector3 | Box | Line): string {
    if ('x' in value) {
      return [value.x, value.y, value.z].join('-');
    }
    if ('minX' in value) {
      return [value.maxX, value.minX, value.maxY, value.minY].join('-');
    }

    if ('points' in value) {
      return getEachKey(value.points[Math.floor(value.points.length / 2)]);
    }

    return '';
  }
</script>

{#each $points as point (getEachKey(point))}
  <T.Mesh
    position.x={point.x}
    position.y={point.y}
    position.z={point.z}
    rotation.x={-Math.PI / 2}
  >
    <T.CircleGeometry args={[0.2, 32]} />
    <T.MeshBasicMaterial color="red" />
  </T.Mesh>
{/each}

{#each $rects as rect, i (getEachKey(rect))}
  <T.Mesh
    position.x={(rect.minX + rect.maxX) / 2}
    position.y={0}
    position.z={(rect.minY + rect.maxY) / 2}
    rotation.x={-Math.PI / 2}
  >
    <T.PlaneGeometry args={[rect.maxX - rect.minX, rect.maxY - rect.minY]} />
    <T.MeshBasicMaterial
      color={new Color().setHSL((i * 1.77) % 1, 1, 0.5)}
      opacity={0.9}
    />
  </T.Mesh>
{/each}

{#each $lines as line (getEachKey(line))}
  <T.Mesh position.y={1}>
    <MeshLineGeometry points={line.points} />
    <MeshLineMaterial
      color={line.color || 'red'}
      linewidth={1}
      attenuate={false}
    />
  </T.Mesh>
{/each}
