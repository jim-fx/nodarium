<script lang="ts">
  import { T, useTask } from '@threlte/core';
  import {
    Color,
    InstancedMesh,
    Matrix4,
    MeshStandardMaterial,
    Quaternion,
    type ShaderMaterial,
    Shape,
    ShapeGeometry,
    UniformsLib,
    UniformsUtils,
    Vector3
  } from 'three';
  import { Reflector } from 'three/examples/jsm/objects/Reflector.js';
  import { createRandom } from '../random';
  import { createRockGeometry } from '../rock-geometry';
  import type { Surface } from '../textures';
  import { COLORS, POND } from './config';
  import { pondOutline } from './pond';
  import { pondFragmentShader, pondVertexShader } from './pond-shader';

  type Props = { stone: Surface };
  const { stone }: Props = $props();

  const WATER_LEVEL = -0.07;

  function createWaterGeometry() {
    const shape = new Shape();
    // a little larger than the hole in the sand, the edge is hidden underneath it
    pondOutline(96, -0.08).forEach(({ x, z }, i) => {
      // the mesh is rotated onto the ground, so shape y becomes -z
      if (i) shape.lineTo(x - POND.x, -(z - POND.z));
      else shape.moveTo(x - POND.x, -(z - POND.z));
    });
    return new ShapeGeometry(shape);
  }

  const water = new Reflector(createWaterGeometry(), {
    color: new Color(COLORS.water),
    textureWidth: 1024,
    textureHeight: 1024,
    clipBias: 0.003,
    shader: {
      name: 'PondShader',
      uniforms: UniformsUtils.merge([UniformsLib.fog, {
        color: { value: null },
        tDiffuse: { value: null },
        textureMatrix: { value: null },
        time: { value: 0 },
        pondCenter: { value: [POND.x, POND.z] },
        pondRadius: { value: POND.radius }
      }]),
      vertexShader: pondVertexShader,
      fragmentShader: pondFragmentShader
    }
  });
  // the Reflector creates its material without fog support
  (water.material as ShaderMaterial).fog = true;

  const uniforms = (water.material as ShaderMaterial).uniforms;
  useTask((delta) => {
    uniforms.time.value += delta;
  });

  /** Pebbles along the shore, they hide the hard edge between sand and water */
  function createPebbles() {
    const random = createRandom(4);
    const outline = pondOutline(140, -0.02);
    const material = new MeshStandardMaterial({ color: COLORS.stone, ...stone });
    const pebbles = new InstancedMesh(createRockGeometry(91, 2), material, outline.length);
    const matrix = new Matrix4();
    const rotation = new Quaternion();
    const axis = new Vector3(0, 1, 0);
    let count = 0;
    for (const { x, z } of outline) {
      // leave gaps so the shore does not look like a string of beads
      if (random() < 0.3) continue;
      const size = 0.05 + Math.pow(random(), 2) * 0.17;
      rotation.setFromAxisAngle(axis, random() * Math.PI * 2);
      matrix.compose(
        new Vector3(x + (random() - 0.5) * 0.12, -0.02, z + (random() - 0.5) * 0.12),
        rotation,
        new Vector3(size * (1 + random() * 0.4), size * 0.7, size)
      );
      pebbles.setMatrixAt(count++, matrix);
    }
    pebbles.count = count;
    pebbles.castShadow = true;
    pebbles.receiveShadow = true;
    return pebbles;
  }
  const pebbles = createPebbles();

  $effect(() => () => {
    water.dispose();
    pebbles.geometry.dispose();
    pebbles.dispose();
  });
</script>

<T is={water} position={[POND.x, WATER_LEVEL, POND.z]} rotation.x={-Math.PI / 2} />
<T is={pebbles} />
