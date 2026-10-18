<script lang="ts">
  import { T } from '@threlte/core';
  import {
    type BufferGeometry,
    DoubleSide,
    InstancedMesh,
    type Material,
    MeshStandardMaterial
  } from 'three';
  import { GROUND_LEVEL } from './config';
  import { growFlora, type Instances } from './flora';
  import {
    createAlliumHeadGeometry,
    createBladeGeometry,
    createFloretGeometry,
    createLeafGeometry,
    createRoseGeometry,
    createStemGeometry
  } from './flora-geometry';
  import { addWind } from './wind';

  type Props = {
    /** where the projects grow, the ground cover keeps away from them */
    plants: { x: number; z: number }[];
  };
  const { plants }: Props = $props();

  const GRASS_BLADES = 70000;

  function material(options: ConstructorParameters<typeof MeshStandardMaterial>[0], wind: number) {
    const result = new MeshStandardMaterial({ side: DoubleSide, ...options });
    addWind(result, wind, GROUND_LEVEL);
    return result;
  }

  const materials = {
    leaf: material({ vertexColors: true, roughness: 0.6 }, 0.012),
    grass: material({ vertexColors: true, roughness: 0.8 }, 0.35),
    rose: material({ vertexColors: true, roughness: 0.5 }, 0.012),
    floret: material({ roughness: 0.55 }, 0.012),
    stem: material({ roughness: 0.7 }, 0.018),
    allium: material({ roughness: 0.6 }, 0.018)
  };

  const geometries = {
    leaf: createLeafGeometry(),
    grass: createBladeGeometry(),
    rose: createRoseGeometry(),
    floret: createFloretGeometry(),
    stem: createStemGeometry(),
    allium: createAlliumHeadGeometry()
  };

  function instanced(
    geometry: BufferGeometry,
    material: Material,
    { matrices, colors }: Instances,
    castShadow = true
  ) {
    const mesh = new InstancedMesh(geometry, material, Math.max(matrices.length, 1));
    mesh.count = matrices.length;
    matrices.forEach((matrix, i) => mesh.setMatrixAt(i, matrix));
    colors.forEach((color, i) => mesh.setColorAt(i, color));
    mesh.castShadow = castShadow;
    mesh.receiveShadow = true;
    // the instances are spread over the whole garden
    mesh.frustumCulled = false;
    return mesh;
  }

  const meshes = $derived.by(() => {
    const flora = growFlora(plants, GRASS_BLADES);
    return [
      instanced(geometries.grass, materials.grass, flora.grass, false),
      instanced(geometries.leaf, materials.leaf, flora.leaves),
      instanced(geometries.rose, materials.rose, flora.roses),
      instanced(geometries.floret, materials.floret, flora.florets),
      instanced(geometries.stem, materials.stem, flora.stems),
      instanced(geometries.allium, materials.allium, flora.heads)
    ];
  });

  $effect(() => {
    const current = meshes;
    return () => current.forEach((mesh) => mesh.dispose());
  });

  $effect(() => () => {
    Object.values(materials).forEach((m) => m.dispose());
    Object.values(geometries).forEach((g) => g.dispose());
  });
</script>

{#each meshes as mesh (mesh.id)}
  <T is={mesh} />
{/each}
