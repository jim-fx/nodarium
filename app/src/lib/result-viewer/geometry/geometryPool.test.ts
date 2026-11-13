import {
  Group,
  type InstancedMesh,
  type Material,
  type Mesh,
  MeshBasicMaterial,
  MeshMatcapMaterial,
  MeshStandardMaterial,
  ShaderMaterial,
  Texture
} from 'three';
import { describe, expect, it } from 'vitest';
import {
  createInstancedGeometryPool,
  createResultPools,
  takeVertexColors,
  withVertexColors
} from './geometryPool';

// [2, vertices, faces, instances, depth, ...faces, ...positions, ...normals, ...matrices]
// with a single triangle, every instance moved along x by its index times `offset`
function instances(count: number, vertexCount = 3, offset = 1) {
  const faces = [0, 1, 2];
  const positions = Array.from({ length: vertexCount * 3 }, (_, i) => i % 3 === 0 ? i / 3 : 0);
  const normals = Array.from({ length: vertexCount * 3 }, (_, i) => i % 3 === 1 ? 1 : 0);
  const matrices = Array.from({ length: count }, (_, i) => [
    1,
    0,
    0,
    0,
    0,
    1,
    0,
    0,
    0,
    0,
    1,
    0,
    i * offset,
    0,
    0,
    1
  ]).flat();

  const data = new Int32Array(5 + 3 + vertexCount * 6 + count * 16);
  data.set([2, vertexCount, 1, count, 0, ...faces]);
  new Float32Array(data.buffer).set([...positions, ...normals, ...matrices], 8);
  return data;
}

function setup() {
  const group = new Group();
  const pool = createInstancedGeometryPool(group, new MeshBasicMaterial());
  const meshes = () => group.children[0].children as InstancedMesh[];
  return { pool, meshes };
}

describe('createInstancedGeometryPool', () => {
  it('keeps every input on its own mesh when a mesh has to grow', () => {
    const { pool, meshes } = setup();
    pool.update([instances(2), instances(2, 4)]);
    pool.update([instances(8), instances(2, 4)]);

    const shown = meshes().map((m) => [m.count, m.geometry.userData.vertexCount]);
    expect(shown).toContainEqual([8, 3]);
    expect(shown).toContainEqual([2, 4]);
  });

  it('shows a mesh again that was hidden for a graph with fewer inputs', () => {
    const { pool, meshes } = setup();
    pool.update([instances(2), instances(2)]);
    pool.update([instances(2)]);
    pool.update([instances(2), instances(2)]);

    expect(meshes().every((m) => m.visible)).toBe(true);
  });

  it('bounds the instances moved by their matrices', () => {
    const { pool, meshes } = setup();
    pool.update([instances(2, 3, 10)]);

    // the triangle spans x from 0 to 2, the second instance sits at x = 10
    expect(meshes()[0].boundingBox?.max.x).toBe(12);
    expect(meshes()[0].boundingSphere?.center.x).toBe(6);
  });

  it('reports the bounds of what is shown and nothing hidden', () => {
    const { pool } = setup();
    pool.update([instances(2, 3, 10), instances(2, 4)]);
    expect(pool.update([instances(2, 4)]).bounds.max.x).toBe(4);
  });

  it('groups every instance by the path it sits on', () => {
    const { pool, meshes } = setup();
    const data = new Int32Array([...instances(2), 0x100_0007, 9]);
    pool.update([data]);
    expect([...meshes()[0].geometry.getAttribute('instanceGroup').array]).toEqual([7, 9]);
    pool.update([instances(3)]);
    expect([...meshes()[0].geometry.getAttribute('instanceGroup').array]).toEqual([0, 0, 0]);
  });

  it('keeps the other inputs on their meshes when one is removed', () => {
    const { pool, meshes } = setup();
    pool.update([instances(2, 3), instances(2, 4), instances(2, 5)]);
    const [a, , c] = meshes();
    pool.update([instances(2, 3), instances(2, 5)]);

    expect(meshes()[0]).toBe(a);
    expect(meshes()[2]).toBe(c);
    expect(meshes().map((m) => m.visible)).toEqual([true, false, true]);
  });
});

// [14, vertices, faces, flags, ...faces, ...positions, ...normals, ...colors?, ...groups] with a
// single triangle, without colors when `color` is null
function myzelMesh(color: number | null = 0xff00_80ff, x = 0, groups = [0, 0, 0]) {
  const colors = color === null ? [] : [color, color, color];
  const data = new Int32Array(4 + 3 + 3 * 6 + colors.length + 3);
  data.set([14, 3, 1, color === null ? 0 : 1, 0, 1, 2]);
  const floats = new Float32Array(data.buffer);
  floats.set([x, 0, 0, x + 1, 0, 0, x, 1, 0], 7);
  floats.set([0, 0, 1, 0, 0, 1, 0, 0, 1], 16);
  data.set([...colors, ...groups], 25);
  return data;
}

// [1, vertices, faces, path id, ...faces, ...positions, ...normals, ...colors?] with a single
// triangle
function plantGeometry(pathId: number, colors: number[] = []) {
  const data = new Int32Array(4 + 3 + 3 * 6 + colors.length);
  data.set([1, 3, 1, pathId, 0, 1, 2]);
  new Float32Array(data.buffer).set([0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 1, 0, 0, 1], 7);
  data.set(colors, 25);
  return data;
}

// instances of a single triangle whose vertices have colors, after the path ids
function coloredInstances(count: number, color: number) {
  const plain = instances(count);
  const data = new Int32Array(plain.length + count + 3);
  data.set(plain);
  data.fill(-1, plain.length, plain.length + count);
  data.fill(color, plain.length + count);
  return data;
}

function shownMeshes(group: Group) {
  return group.children.flatMap((pool) => pool.children as Mesh[]).filter((m) => m.visible);
}

describe('createResultPools', () => {
  it('shows meshes with their colors next to plain geometry', () => {
    const group = new Group();
    const material = takeVertexColors(new MeshBasicMaterial());
    const pools = createResultPools(group, material);
    const stats = pools.update([myzelMesh(0xffff_ffff, 2)]);
    expect(stats.totalVertices).toBe(3);
    expect(stats.bounds.max.x).toBe(3);

    const meshes = shownMeshes(group);
    expect(meshes).toHaveLength(1);
    const colors = meshes[0].geometry.getAttribute('color');
    expect(colors.getX(0)).toBeCloseTo(1);
    expect((meshes[0].material as MeshBasicMaterial).vertexColors).toBe(true);
    expect(material.vertexColors).toBe(false);
  });

  it('keeps the alpha of the colors, vertices without a color of their own have 0', () => {
    const group = new Group();
    createResultPools(group, new MeshBasicMaterial()).update([myzelMesh(0x00ff_ffff)]);
    const colors = shownMeshes(group)[0].geometry.getAttribute('color');
    expect(colors.itemSize).toBe(4);
    expect([colors.getX(0), colors.getY(0), colors.getZ(0), colors.getW(0)]).toEqual([1, 1, 1, 0]);
  });

  it('keeps the look of the material where a vertex has no color', () => {
    const compile = (material: Material, fragmentShader: string) => {
      const shader = { fragmentShader, vertexShader: '', uniforms: {} };
      material.onBeforeCompile(shader as never, undefined as never);
      return shader.fragmentShader;
    };
    // a tint of its own still runs, the colors mix in by their alpha
    const tinted = takeVertexColors(new MeshStandardMaterial());
    tinted.onBeforeCompile = (shader) => {
      shader.fragmentShader += ' tinted';
    };
    const tintedColors = withVertexColors(tinted);
    expect(tintedColors.vertexColors).toBe(true);
    expect(compile(tintedColors, '#include <color_fragment>')).toMatch(
      /mix\(diffuseColor.rgb, diffuseColor.rgb \* vColor.rgb, vColor.a\)[\s\S]* tinted$/
    );
    expect(tintedColors.customProgramCacheKey()).not.toBe(tinted.customProgramCacheKey());

    // a color gets the light of the environment without the ambient light and neutral tone mapping
    const lit = compile(
      withVertexColors(takeVertexColors(new MeshStandardMaterial())),
      '#include <lights_fragment_end>\n#include <tonemapping_fragment>'
    );
    expect(lit).toMatch(/irradiance \*= 1.0 - vColor.a;\s*#endif\s*#include <lights_fragment_end>/);
    expect(lit).toMatch(/mix\(\s*toneMapping\(gl_FragColor.rgb\),\s*NeutralToneMapping\(/);

    // the matcap stays for vertices without a color
    const matcap = takeVertexColors(new MeshMatcapMaterial({ matcap: new Texture() }));
    const matcapColors = withVertexColors(matcap) as MeshMatcapMaterial;
    expect(matcapColors.matcap).toBe(matcap.matcap);
    expect(
      compile(
        matcapColors,
        '#include <color_fragment>\nvec3 outgoingLight = diffuseColor.rgb * matcapColor.rgb;'
      )
    ).toMatch(/^\n\s*vec3 colorShade[\s\S]*mix\(matcapColor.rgb, colorShade, vColor.a\)/);
  });

  it('ignores the colors with materials that do not take them', () => {
    for (
      const material of [new MeshBasicMaterial(), new MeshMatcapMaterial(), new ShaderMaterial()]
    ) {
      expect(withVertexColors(material)).toBe(material);
      expect(material.vertexColors).toBe(false);
    }

    const group = new Group();
    const material = new MeshStandardMaterial();
    const pools = createResultPools(group, material);
    pools.update([myzelMesh(0xffff_ffff)]);
    expect(shownMeshes(group)[0].material).toBe(material);

    // switching to a material that takes colors shows them again
    const colored = takeVertexColors(new MeshStandardMaterial());
    pools.setMaterial(colored);
    expect((shownMeshes(group)[0].material as Material).vertexColors).toBe(true);
    pools.setMaterial(material);
    expect(shownMeshes(group)[0].material).toBe(material);
  });

  it('updates the colors when only they changed', () => {
    const group = new Group();
    const pools = createResultPools(group, new MeshBasicMaterial());
    pools.update([myzelMesh(0xffff_ffff)]);
    pools.update([myzelMesh(0xff00_0000)]);
    const mesh = shownMeshes(group)[0];
    expect(mesh.geometry.getAttribute('color').getX(0)).toBe(0);
  });

  it('shows meshes without colors with the material as it is', () => {
    const group = new Group();
    const material = takeVertexColors(new MeshMatcapMaterial({ matcap: new Texture() }));
    const pools = createResultPools(group, material);
    pools.update([myzelMesh(0xffff_ffff)]);
    pools.update([myzelMesh(null)]);
    const mesh = shownMeshes(group)[0];
    expect(mesh.geometry.getAttribute('color')).toBeUndefined();
    expect(mesh.material).toBe(material);

    const other = new MeshBasicMaterial();
    pools.setMaterial(other);
    expect(mesh.material).toBe(other);
  });

  it('gives every vertex the group of its mesh', () => {
    const group = new Group();
    const pools = createResultPools(group, new MeshBasicMaterial());
    pools.update([myzelMesh(null, 0, [0, 3, 5])]);
    expect([...shownMeshes(group)[0].geometry.getAttribute('group').array]).toEqual([0, 3, 5]);
    pools.update([myzelMesh(null, 0, [1, 1, 1])]);
    expect([...shownMeshes(group)[0].geometry.getAttribute('group').array]).toEqual([1, 1, 1]);
  });

  it('groups plant geometry by its path, keeping 24 bits of the id', () => {
    const group = new Group();
    const pools = createResultPools(group, new MeshBasicMaterial());
    pools.update([plantGeometry(0x7f12_3456)]);
    expect([...shownMeshes(group)[0].geometry.getAttribute('group').array]).toEqual(
      [0x12_3456, 0x12_3456, 0x12_3456]
    );
  });

  it('shows plant geometry and instances with their colors', () => {
    const group = new Group();
    const material = takeVertexColors(new MeshBasicMaterial());
    const pools = createResultPools(group, material);
    pools.update([plantGeometry(5, [-1, -1, -1]), coloredInstances(2, 0xff00_00ff), instances(1)]);
    const meshes = shownMeshes(group);
    expect(meshes).toHaveLength(3);
    const [geometry, colored, plain] = meshes;
    expect(geometry.geometry.getAttribute('color').getX(0)).toBeCloseTo(1);
    expect((geometry.material as Material).vertexColors).toBe(true);
    expect(colored.geometry.getAttribute('color').getX(0)).toBeCloseTo(1);
    expect(colored.geometry.getAttribute('color').getY(0)).toBeCloseTo(0);
    expect((colored.material as Material).vertexColors).toBe(true);
    expect(plain.geometry.getAttribute('color')).toBeUndefined();
    expect(plain.material).toBe(material);

    // without colors the same instances take the material as it is again
    pools.update([plantGeometry(5), instances(2), instances(1)]);
    expect(shownMeshes(group)[1].geometry.getAttribute('color')).toBeUndefined();
    expect(shownMeshes(group)[1].material).toBe(material);
  });
});
