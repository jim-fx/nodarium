<script lang="ts">
  import { colors } from '$lib/graph-interface/graph/colors.svelte';
  import { T, useTask, useThrelte } from '@threlte/core';
  import { Grid } from '@threlte/extras';
  import { Box3, type BufferGeometry, type Group, Mesh, MeshBasicMaterial, Vector3 } from 'three';
  import { appSettings } from '../settings/app-settings.svelte';
  import Camera from './Camera.svelte';
  import Debug from './Debug.svelte';

  const { renderStage, invalidate: _invalidate } = useThrelte();

  type Props = {
    fps: number[];
    debugData?: Record<number, { type: string; data: Int32Array }>;
    scene: Group;
    centerCamera: boolean;
  };

  let {
    centerCamera,
    fps = $bindable(),
    scene = $bindable(),
    debugData
  }: Props = $props();

  let geometries = $state.raw<BufferGeometry[]>([]);
  let center = $state(new Vector3(0, 4, 0));

  useTask(
    (delta) => {
      fps.push(1 / delta);
      fps = fps.slice(-100);
    },
    { stage: renderStage, autoInvalidate: false }
  );

  export const invalidate = function() {
    if (scene) {
      const geos: BufferGeometry[] = [];
      scene.traverse(function(child) {
        if (isMesh(child)) {
          geos.push(child.geometry);
        }
      });
      geometries = geos;
    }

    if (geometries && scene && centerCamera) {
      const aabb = new Box3().setFromObject(scene);
      center = aabb
        .getCenter(new Vector3())
        .max(new Vector3(-4, -4, -4))
        .min(new Vector3(4, 4, 4));
    }
    _invalidate();
  };

  function isMesh(child: unknown): child is Mesh {
    return (
      child !== null
      && typeof child === 'object'
      && 'isObject3D' in child
      && child.isObject3D === true
      && 'material' in child
    );
  }

  function isMatCapMaterial(material: unknown): material is MeshBasicMaterial {
    return (
      material !== null
      && typeof material === 'object'
      && 'isMaterial' in material
      && material.isMaterial === true
      && 'matcap' in material
    );
  }

  $effect(() => {
    const wireframe = appSettings.value.debug.wireframe;
    scene.traverse(function(child) {
      if (isMesh(child) && isMatCapMaterial(child.material) && child.visible) {
        child.material.wireframe = wireframe;
      }
    });
    _invalidate();
  });
</script>

<Camera {center} {centerCamera} />

<Debug {debugData} />

{#if appSettings.value.showGrid}
  <Grid
    cellColor={colors['outline']}
    cellThickness={0.7}
    infiniteGrid
    sectionThickness={0.7}
    sectionDistance={2}
    sectionColor={colors['outline']}
    fadeDistance={50}
    fadeStrength={10}
    fadeOrigin={new Vector3(0, 0, 0)}
  />
{/if}
<T.Group bind:ref={scene}></T.Group>
