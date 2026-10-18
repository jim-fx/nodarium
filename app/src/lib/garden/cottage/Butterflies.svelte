<script lang="ts">
  import { T, useTask } from '@threlte/core';
  import { DoubleSide, Group, Mesh, MeshStandardMaterial, Shape, ShapeGeometry } from 'three';
  import { BED_DEPTH, BED_WIDTH } from '../config';
  import { createRandom } from '../random';

  const random = createRandom(8);

  function createWingGeometry() {
    const shape = new Shape();
    shape.moveTo(0, 0);
    shape.bezierCurveTo(0.35, 0.55, 1.05, 0.65, 0.95, 0.2);
    shape.bezierCurveTo(0.9, -0.05, 0.7, -0.15, 0.55, -0.12);
    shape.bezierCurveTo(0.75, -0.45, 0.45, -0.75, 0.2, -0.45);
    shape.bezierCurveTo(0.1, -0.3, 0.05, -0.1, 0, 0);
    return new ShapeGeometry(shape, 8);
  }

  const wing = createWingGeometry();
  const butterflies = ['#fff8e6', '#f6e27a', '#f4a24a', '#fff8e6', '#e9eefc'].map((color) => {
    const material = new MeshStandardMaterial({ color, side: DoubleSide, roughness: 0.6 });
    const body = new Group();
    body.scale.setScalar(0.13 + random() * 0.05);
    // the wings are hinged at the body, the shape is drawn for the right one
    const wings = [1, -1].map((side) => {
      const hinge = new Group();
      hinge.scale.x = side;
      const mesh = new Mesh(wing, material);
      mesh.rotation.x = Math.PI / 2;
      mesh.castShadow = true;
      hinge.add(mesh);
      body.add(hinge);
      return hinge;
    });
    return { material, body, wings, seed: random() * 100, speed: 0.08 + random() * 0.06 };
  });

  let time = 0;
  useTask((delta) => {
    time += delta;
    for (const b of butterflies) {
      const t = time * b.speed + b.seed;
      // wandering loops over the garden, now and then dipping down to the flowers
      const x = Math.sin(t * 1.3) * BED_WIDTH * 0.55 + Math.sin(t * 3.1) * 1.5;
      const z = Math.cos(t * 0.9) * (BED_DEPTH * 0.5 + 2) + Math.sin(t * 2.3) * 1.2;
      const y = 1.4 + Math.sin(t * 2.1) * 0.8 + Math.sin(t * 7.3) * 0.15;
      const dx = x - b.body.position.x, dz = z - b.body.position.z;
      b.body.position.set(x, y, z);
      if (dx * dx + dz * dz > 1e-8) b.body.rotation.y = Math.atan2(dx, dz);
      const flap = Math.sin(time * (14 + b.seed % 4)) * 0.9 + 0.35;
      b.wings[0].rotation.z = flap;
      b.wings[1].rotation.z = -flap;
    }
  });

  $effect(() => () => {
    wing.dispose();
    butterflies.forEach((b) => b.material.dispose());
  });
</script>

{#each butterflies as b, i (i)}
  <T is={b.body} />
{/each}
