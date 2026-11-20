import { GEOMETRY_FLAGS } from '@nodarium/runtime';
import { concatEncodedArrays, PATH_HEADER_SIZE } from '@nodarium/utils';
import {
  ArrowHelper,
  Group,
  InstancedMesh,
  Line,
  LineSegments,
  type Material,
  Mesh,
  MeshMatcapMaterial,
  MeshStandardMaterial,
  Points,
  ShaderMaterial,
  Vector2,
  Vector3
} from 'three';
import { describe, expect, it } from 'vitest';
import {
  clipPoints,
  createDebugScene,
  type DebugEntry,
  drawsSomething,
  inFront,
  isShown
} from './debug';

/** Draws one entry into `group` with a scene of its own, like the viewer does on its first run */
function updateDebugScene(group: Group, entry: DebugEntry, material?: Material) {
  return createDebugScene(group, material ?? new MeshStandardMaterial()).update(entry);
}

/** The meshes and instanced meshes that show something */
function surfaces(group: Group) {
  const meshes: Mesh[] = [];
  const instanced: InstancedMesh[] = [];
  group.traverse((child) => {
    if (!child.visible) return;
    if (child instanceof InstancedMesh) instanced.push(child);
    else if (child instanceof Mesh && !(child.parent instanceof ArrowHelper)) meshes.push(child);
  });
  return { meshes, instanced };
}

function wrap(item: Int32Array) {
  const data = new Int32Array(item.length + 4);
  data.set([0, item.length + 1]);
  data.set(item, 2);
  data.set([1, 1], item.length + 2);
  return data;
}

// [0, depth, id, parent id, parent alpha, creator, index, ...x y z thickness]
function path(depth: number, points: number[][]) {
  const data = new Int32Array(PATH_HEADER_SIZE + points.length * 4);
  data.set([0, depth, 0, -1, 0, 0, 0]);
  const floats = new Float32Array(data.buffer);
  // a full-grown path
  floats[7] = 1;
  floats.set(points.flat(), PATH_HEADER_SIZE);
  return data;
}

// [2, vertices, faces, instances, creator, ...faces, ...positions, ...normals, ...matrices]
function instances(count: number) {
  const data = new Int32Array(5 + 3 + 18 + count * 16);
  data.set([2, 3, 1, count, 0, 0, 1, 2]);
  const identity = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1];
  new Float32Array(data.buffer).set(
    [0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1, 0, ...Array(count).fill(identity).flat()],
    8
  );
  return data;
}

// [4, parent id, depth, index, x y z, dx dy dz, tx ty tz, scale, alpha]
function point(position: number[], direction: number[], scale = 0.1) {
  const data = new Int32Array(15);
  data.set([4, 7, 1, 0]);
  new Float32Array(data.buffer).set([...position, ...direction, 0, 1, 0, scale, 0.5], 4);
  return data;
}

/**
 * [1, vertices, faces, path id, flags, ...faces, ...positions, ...normals, ...colors?, ...groups?]
 * with one triangle from (0, 0, 0) to (1, 1, 0)
 */
function triangle({ groups = false, colors = false } = {}) {
  const data = new Int32Array(5 + 3 + 18 + (colors ? 3 : 0) + (groups ? 3 : 0));
  data.set([
    1,
    3,
    1,
    -1,
    (colors ? GEOMETRY_FLAGS.colors : 0) | (groups ? GEOMETRY_FLAGS.groups : 0)
  ]);
  data.set([0, 1, 2], 5);
  new Float32Array(data.buffer).set([0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 1, 0, 0, 1], 8);
  if (colors) data.set([0xff0000ff, 0xff00ff00, 0xffff0000], 26);
  return data;
}

function collect(group: Group) {
  const lines: Line[] = [];
  const points: Points[] = [];
  const segments: LineSegments[] = [];
  const arrows: ArrowHelper[] = [];
  group.traverse((child) => {
    if (child instanceof ArrowHelper) arrows.push(child);
    else if (child instanceof Points) points.push(child);
    else if (child instanceof LineSegments) segments.push(child);
    else if (child instanceof Line && !(child.parent instanceof ArrowHelper)) lines.push(child);
  });
  return { lines, points, segments, arrows };
}

describe('updateDebugScene', () => {
  it('draws only the paths of a path output as lines, instances separately', () => {
    const group = new Group();
    const data = concatEncodedArrays([
      wrap(path(1, [[0, 0, 0, 0.1], [0, 1, 0, 0.1]])),
      wrap(path(2, [[0, 1, 0, 0.05], [1, 1, 0, 0.05], [2, 1, 0, 0.05]])),
      wrap(instances(4))
    ]);

    updateDebugScene(group, { type: 'path', data });

    const { lines, points, arrows } = collect(group);
    expect(lines).toHaveLength(2);
    expect(points[0].geometry.getAttribute('position').count).toBe(5);
    // the instances are drawn as themselves, not as arrows
    expect(arrows).toHaveLength(0);
    expect(surfaces(group).instanced[0].count).toBe(4);
  });

  it('draws a single geometry that is not wrapped in a list', () => {
    const group = new Group();
    updateDebugScene(group, { type: 'path', data: wrap(instances(2)) });
    expect(surfaces(group).instanced[0].count).toBe(2);
  });

  it('places the instances with their matrices', () => {
    const group = new Group();
    const data = instances(2);
    const floats = new Float32Array(data.buffer);
    // the second instance moved to (1, 2, 13)
    floats.set([1, 2, 13], 26 + 16 + 12);
    updateDebugScene(group, { type: 'path', data: wrap(data) });

    const [mesh] = surfaces(group).instanced;
    expect([...mesh.instanceMatrix.array.subarray(16 + 12, 16 + 15)]).toEqual([1, 2, 13]);
  });

  it('draws a dot and an arrow along its direction for every point', () => {
    const group = new Group();
    const data = concatEncodedArrays([
      wrap(point([0, 1, 0], [1, 0, 0])),
      wrap(point([0, 2, 0], [-1, 0, 0], 0.2))
    ]);

    updateDebugScene(group, { type: 'points', data });

    const { points, arrows, segments } = collect(group);
    expect(points[0].geometry.getAttribute('position').count).toBe(2);
    expect(arrows.map((a) => a.position.toArray())).toEqual([[0, 1, 0], [0, 2, 0]]);
    const directions = arrows.map((a) => new Vector3(0, 1, 0).applyQuaternion(a.quaternion));
    expect(directions[0].distanceTo(new Vector3(1, 0, 0))).toBeLessThan(1e-6);
    expect(directions[1].distanceTo(new Vector3(-1, 0, 0))).toBeLessThan(1e-6);
    // a thicker path gets a longer arrow
    expect(arrows[0].cone.position.y).toBeCloseTo(0.5);
    expect(arrows[1].cone.position.y).toBeCloseTo(1);
    // a line along the tangent of every point
    expect(segments[0].geometry.getAttribute('position').count).toBe(4);
  });

  it('draws a single point that is not wrapped in a list', () => {
    const group = new Group();
    updateDebugScene(group, { type: 'points', data: wrap(point([0, 0, 0], [0, 0, 1])) });
    expect(collect(group).arrows).toHaveLength(1);
  });

  it('skips single values and clears what it showed before', () => {
    const group = new Group();
    const scene = createDebugScene(group, new MeshStandardMaterial());
    scene.update({ type: 'path', data: wrap(path(1, [[0, 0, 0, 1], [0, 1, 0, 1]])) });
    scene.update({ type: 'geometry', data: wrap(triangle()) });
    expect(collect(group).lines).toHaveLength(0);
    expect(surfaces(group).meshes).toHaveLength(1);

    const bounds = scene.update({ type: 'float', data: new Int32Array([5]) });
    expect(bounds.isEmpty()).toBe(true);
    expect(surfaces(group).meshes).toHaveLength(0);
    expect(collect(group).lines).toHaveLength(0);
    expect(drawsSomething({ type: 'float', data: new Int32Array([5]) })).toBe(false);
  });

  it('draws a geometry with groups and the preview of a pattern, but not a raw pattern', () => {
    const mesh = wrap(triangle({ groups: true }));
    const group = new Group();
    const scene = createDebugScene(group, new MeshStandardMaterial());
    scene.update({ type: 'geometry', data: mesh });
    expect(surfaces(group).meshes).toHaveLength(1);
    expect(surfaces(group).meshes[0].geometry.getAttribute('position').count).toBe(3);

    // the runtime turns a pattern into the list of the output node
    scene.update({ type: 'pattern', data: Int32Array.of(0, 1, ...mesh, 1, 1) });
    expect(surfaces(group).meshes).toHaveLength(1);

    scene.update({ type: 'pattern', data: Int32Array.of(15, 0, 0) });
    expect(surfaces(group).meshes).toHaveLength(0);
  });
});

describe('the surfaces of the debug node', () => {
  it('are drawn with the material of the viewer, in front and with the depth test', () => {
    const material = new MeshStandardMaterial({ color: '#336633' });
    const group = new Group();
    const scene = createDebugScene(group, material);
    const bounds = scene.update({ type: 'geometry', data: wrap(triangle()) });
    expect(bounds.min.toArray()).toEqual([0, 0, 0]);
    expect(bounds.max.toArray()).toEqual([1, 1, 0]);

    const [mesh] = surfaces(group).meshes;
    const shown = mesh.material as MeshStandardMaterial;
    expect(shown).toBeInstanceOf(MeshStandardMaterial);
    expect(shown.color.getHexString()).toBe(material.color.getHexString());
    expect(shown.depthTest).toBe(true);
    expect(shown.wireframe).toBe(false);
    expect(shown.polygonOffset).toBe(true);
    expect(shown.polygonOffsetFactor).toBeLessThan(0);
    expect(shown.customProgramCacheKey()).toBe(material.customProgramCacheKey());

    const next = new MeshStandardMaterial({ color: '#aa0000' });
    scene.setMaterial(next);
    expect((mesh.material as MeshStandardMaterial).color.getHexString()).toBe('aa0000');
  });

  it('keep the colors of the vertices, also with a material that leaves them out in the result', () => {
    const material = new MeshMatcapMaterial();
    const group = new Group();
    createDebugScene(group, material).update({
      type: 'pattern',
      data: wrap(triangle({ colors: true, groups: true }))
    });
    const [mesh] = surfaces(group).meshes;
    expect(mesh.geometry.getAttribute('color')).toBeDefined();
    expect((mesh.material as MeshMatcapMaterial).vertexColors).toBe(true);
    expect(material.vertexColors).toBe(false);
  });

  it('share the uniforms of a shader material, its height range is set on them', () => {
    const material = new ShaderMaterial({ uniforms: { uHeightRange: { value: new Vector2() } } });
    const front = inFront(material) as ShaderMaterial;
    expect(front.uniforms).toBe(material.uniforms);
    expect(inFront(material)).toBe(front);
  });
});

const pathItems = (n: number) =>
  Array.from({ length: n }, (_, i) => {
    const p = path(1, [[i, 0, 0, 0.1], [i, 1, 0, 0.1]]);
    p[2] = 1000 + i;
    return wrap(p);
  });

const drawnStarts = (group: Group) =>
  collect(group).lines.map((l) => l.geometry.getAttribute('position').getX(0));

describe('the amount of the debug node', () => {
  it('only adds items when it grows', () => {
    const keys = Array.from({ length: 2000 }, (_, i) => i);
    let previous = new Set<number>();
    for (const amount of [0, 0.05, 0.2, 0.5, 0.8, 1]) {
      const shown = new Set(keys.filter((k) => isShown(k, amount)));
      for (const k of previous) expect(shown.has(k)).toBe(true);
      expect(shown.size / keys.length).toBeCloseTo(amount, 1);
      previous = shown;
    }
  });

  it('draws the same paths every time, a share of them', () => {
    const data = concatEncodedArrays(pathItems(200));
    const draw = (amount: number) => {
      const group = new Group();
      updateDebugScene(group, { type: 'path', data, amount });
      return drawnStarts(group);
    };
    const few = draw(0.2);
    expect(draw(0.2)).toEqual(few);
    expect(few.length).toBeGreaterThan(20);
    expect(few.length).toBeLessThan(60);
    const more = draw(0.5);
    expect(few.every((x) => more.includes(x))).toBe(true);
    expect(draw(0)).toHaveLength(0);
  });

  it('applies to instances, points and meshes', () => {
    const group = new Group();
    const data = instances(400);
    // a path id for every instance
    const withIds = new Int32Array(data.length + 400);
    withIds.set(data);
    withIds.set(Array.from({ length: 400 }, (_, i) => 7000 + i), data.length);
    updateDebugScene(group, { type: 'path', data: wrap(withIds), amount: 0.1 });
    const [mesh] = surfaces(group).instanced;
    expect(mesh.count).toBeGreaterThan(10);
    expect(mesh.count).toBeLessThan(80);
    // the path ids stay with the instances they belong to
    const ids: Int32Array = mesh.userData.pathIds;
    expect(ids).toHaveLength(mesh.count);
    expect([...ids].every((id) => id >= 7000 && id < 7400)).toBe(true);

    const pointData = concatEncodedArrays(
      Array.from({ length: 200 }, (_, i) => {
        const p = point([0, i, 0], [1, 0, 0]);
        p[3] = i;
        return wrap(p);
      })
    );
    updateDebugScene(group, { type: 'points', data: pointData, amount: 0.1 });
    expect(collect(group).arrows.length).toBeLessThan(60);
  });

  it('draws a share of the faces of a geometry with groups', () => {
    const faces = 300;
    const inner = new Int32Array(5 + faces * 3 + 3 * 7);
    inner.set([1, 3, faces, -1, 4]);
    for (let f = 0; f < faces; f++) inner.set([0, 1, 2], 5 + f * 3);
    const mesh = wrap(inner);
    const drawn = (amount: number) => {
      const group = new Group();
      updateDebugScene(group, { type: 'geometry', data: mesh, amount });
      return surfaces(group).meshes[0].geometry.index!.count / 3;
    };
    expect(drawn(1)).toBe(faces);
    expect(drawn(0.1)).toBeGreaterThan(10);
    expect(drawn(0.1)).toBeLessThan(60);
  });
});

describe('the clip of the debug node', () => {
  it('cuts a path along its length', () => {
    const points = [[0, 0, 0, 1], [0, 1, 0, 1], [0, 3, 0, 3]].map(
      ([x, y, z, t]) => [new Vector3(x, y, z), t] as [Vector3, number]
    );
    const half = clipPoints(points, 0.5);
    expect(half.map(([p]) => p.toArray())).toEqual([[0, 0, 0], [0, 1, 0], [0, 1.5, 0]]);
    expect(half[2][1]).toBeCloseTo(1.5);
    expect(clipPoints(points, 1)).toBe(points);
    expect(clipPoints(points, 0)).toEqual([]);
  });

  it('ends the line, the dots and the rings at the cut', () => {
    const data = wrap(path(1, [[0, 0, 0, 0.1], [0, 1, 0, 0.1], [0, 2, 0, 0.1], [0, 3, 0, 0.1]]));
    const group = new Group();
    updateDebugScene(group, { type: 'path', data, clip: 0.5 });
    const { lines: drawn, points, segments } = collect(group);
    const line = drawn[0].geometry.getAttribute('position');
    expect(line.count).toBe(3);
    expect(line.getY(2)).toBeCloseTo(1.5);
    const dots = points[0].geometry.getAttribute('position');
    expect(dots.count).toBe(3);
    expect(Math.max(...Array.from({ length: dots.count }, (_, i) => dots.getY(i)))).toBeCloseTo(
      1.5
    );
    const rings = segments[0].geometry.getAttribute('position');
    expect(rings.count).toBe(3 * 12 * 2);
    for (let i = 0; i < rings.count; i++) expect(rings.getY(i)).toBeLessThan(1.5 + 1e-6);
  });
});

describe('the defaults of the debug node', () => {
  it('draw everything as it was', () => {
    const data = concatEncodedArrays([
      ...pathItems(30),
      wrap(instances(5)),
      wrap(point([0, 1, 0], [1, 0, 0]))
    ]);
    const positions = (group: Group) => {
      const out: number[][] = [];
      group.traverse((child) => {
        const attribute = (child as Mesh).geometry?.getAttribute('position');
        if (attribute) out.push([...attribute.array]);
      });
      return out;
    };
    const before = new Group();
    updateDebugScene(before, { type: 'path', data });
    const after = new Group();
    updateDebugScene(after, { type: 'path', data, amount: 1, clip: 1 });
    expect(positions(after)).toEqual(positions(before));
    expect(positions(before).length).toBeGreaterThan(30);
  });
});
