<script lang="ts">
  import { T } from '@threlte/core';
  import { useGltf } from '@threlte/extras';
  import {
    type Box3,
    Group,
    Mesh,
    type MeshStandardMaterial,
    type Object3D,
    TextureLoader
  } from 'three';
  import { BED_DEPTH, BED_WIDTH } from '../config';
  import { createRandom } from '../random';
  import { FLOOR_LEVEL } from './config';

  const fern = useGltf('/garden/models/fern_02/fern_02.gltf');
  // the gltf of poly haven ships jpegs, the cut out of the leaves is a separate map
  const fernAlpha = new TextureLoader().load(
    '/garden/models/fern_02/textures/fern_02_alpha_1k.jpg'
  );
  fernAlpha.flipY = false;
  const stones = useGltf('/garden/models/rock_moss_set_02/rock_moss_set_02.gltf');

  /** distance between the bed and the closest scenery */
  const CLEARANCE = 0.9;

  /** The meshes of a model set, each one moved onto the origin */
  function variants(scene: Object3D) {
    const meshes: Mesh[] = [];
    scene.traverse((child) => {
      if (child instanceof Mesh) meshes.push(child);
    });
    return meshes.map((mesh) => {
      mesh.geometry.computeBoundingBox();
      const box = mesh.geometry.boundingBox as Box3;
      return {
        mesh,
        ground: box.min.y,
        width: Math.max(box.max.x - box.min.x, box.max.z - box.min.z)
      };
    });
  }

  /**
   * Picks a spot around the bed, most of them behind and next to it.
   * A few end up in front, close to the camera, where they are out of focus.
   */
  function spot(random: () => number) {
    for (;;) {
      const angle = random() * Math.PI * 2;
      const distance = 0.6 + Math.pow(random(), 1.6) * 5;
      const x = Math.cos(angle) * (BED_WIDTH / 2 + distance);
      const z = -Math.sin(angle) * (BED_DEPTH / 2 + distance);
      const inFront = z > BED_DEPTH / 2;
      const outside = Math.abs(x) > BED_WIDTH / 2 + CLEARANCE
        || Math.abs(z) > BED_DEPTH / 2 + CLEARANCE;
      // keep the view onto the bed open, only its front corners get some company
      if (!outside || (inFront && (Math.abs(x) < BED_WIDTH / 2 - 1.5 || random() > 0.35))) continue;
      return { x, z };
    }
  }

  function scatter(
    scene: Object3D,
    count: number,
    seed: number,
    size: [number, number]
  ) {
    const random = createRandom(seed);
    const options = variants(scene);
    const group = new Group();
    for (let i = 0; i < count; i++) {
      const { mesh, ground, width } = options[Math.floor(random() * options.length)];
      const { x, z } = spot(random);
      const instance = new Mesh(mesh.geometry, mesh.material);
      const scale = (size[0] + random() * (size[1] - size[0])) / width;
      instance.scale.setScalar(scale);
      instance.position.set(x, FLOOR_LEVEL - ground * scale, z);
      instance.rotation.y = random() * Math.PI * 2;
      instance.castShadow = true;
      instance.receiveShadow = true;
      group.add(instance);
    }
    return group;
  }

  const ferns = $derived.by(() => {
    if (!$fern) return;
    $fern.scene.traverse((child) => {
      if (!(child instanceof Mesh)) return;
      const material = child.material as MeshStandardMaterial;
      material.alphaMap = fernAlpha;
      material.needsUpdate = true;
    });
    return scatter($fern.scene, 34, 3, [1.4, 2.6]);
  });
  const rocks = $derived($stones && scatter($stones.scene, 14, 5, [0.6, 1.5]));
</script>

{#if ferns}
  <T is={ferns} />
{/if}
{#if rocks}
  <T is={rocks} />
{/if}
