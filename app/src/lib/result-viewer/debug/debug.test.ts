import { concatEncodedArrays } from '@nodarium/utils';
import { ArrowHelper, Group, Line, LineSegments, type Mesh, Points, Vector3 } from 'three';
import { describe, expect, it } from 'vitest';
import { clipPoints, isShown, selectLines, updateDebugScene } from './debug';

function wrap(item: Int32Array) {
  const data = new Int32Array(item.length + 4);
  data.set([0, item.length + 1]);
  data.set(item, 2);
  data.set([1, 1], item.length + 2);
  return data;
}

// [0, depth, id, parent id, parent alpha, creator, index, ...x y z thickness]
function path(depth: number, points: number[][]) {
  const data = new Int32Array(7 + points.length * 4);
  data.set([0, depth, 0, -1, 0, 0, 0]);
  new Float32Array(data.buffer).set(points.flat(), 7);
  return data;
}

// [2, vertices, faces, instances, depth, ...faces, ...positions, ...normals, ...matrices]
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

    updateDebugScene(group, { 1: { type: 'path', data } });

    const { lines, points, arrows } = collect(group);
    expect(lines).toHaveLength(2);
    expect(points[0].geometry.getAttribute('position').count).toBe(5);
    // an arrow for every instance
    expect(arrows).toHaveLength(4);
  });

  it('draws a single geometry that is not wrapped in a list', () => {
    const group = new Group();
    updateDebugScene(group, { 1: { type: 'path', data: wrap(instances(2)) } });
    expect(collect(group).arrows).toHaveLength(2);
  });

  it('points an instance along its -z axis, as long as its size, scaled up', () => {
    const group = new Group();
    const data = instances(2);
    // scale 2 at (1, 2, 3) and scale 1 at (1, 2, 13)
    new Float32Array(data.buffer).set(
      [
        2,
        0,
        0,
        0,
        0,
        2,
        0,
        0,
        0,
        0,
        2,
        0,
        1,
        2,
        3,
        1,
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
        1,
        2,
        13,
        1
      ],
      26
    );
    updateDebugScene(group, { 1: { type: 'path', data: wrap(data) } });

    const arrows = collect(group).arrows;
    expect(arrows.map((a) => a.position.toArray())).toEqual([[1, 2, 3], [1, 2, 13]]);
    // an arrow points along +y until it is rotated
    for (const arrow of arrows) {
      const direction = new Vector3(0, 1, 0).applyQuaternion(arrow.quaternion);
      expect(direction.distanceTo(new Vector3(0, 0, -1))).toBeLessThan(1e-6);
    }
    expect(arrows.map((a) => a.cone.position.y)).toEqual([200, 100]);
  });

  it('draws a dot and an arrow along its direction for every point', () => {
    const group = new Group();
    const data = concatEncodedArrays([
      wrap(point([0, 1, 0], [1, 0, 0])),
      wrap(point([0, 2, 0], [-1, 0, 0], 0.2))
    ]);

    updateDebugScene(group, { 1: { type: 'points', data } });

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
    updateDebugScene(group, { 1: { type: 'points', data: wrap(point([0, 0, 0], [0, 0, 1])) } });
    expect(collect(group).arrows).toHaveLength(1);
  });

  it('skips single values and clears the previous result', () => {
    const group = new Group();
    updateDebugScene(group, {
      1: { type: 'path', data: wrap(path(1, [[0, 0, 0, 1], [0, 1, 0, 1]])) }
    });
    expect(group.children).not.toHaveLength(0);

    updateDebugScene(group, { 1: { type: 'float', data: new Int32Array([5]) } });
    expect(group.children).toHaveLength(0);
  });

  it('draws a Myzel mesh and the preview of a pattern, but not a raw pattern', () => {
    // [14, vertices, faces, flags, ...faces, ...positions, ...normals, ...groups] with one triangle
    const mesh = new Int32Array(4 + 3 + 21);
    mesh.set([14, 3, 1, 0, 0, 1, 2]);
    new Float32Array(mesh.buffer).set([0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 1, 0, 0, 1], 7);
    const meshes = (group: Group) =>
      group.children.flatMap((g) => g.children.flatMap((pool) => pool.children as Mesh[]));

    const group = new Group();
    updateDebugScene(group, { 1: { type: 'mesh', data: mesh } });
    expect(meshes(group)).toHaveLength(1);
    expect(meshes(group)[0].geometry.getAttribute('position').count).toBe(3);

    // the runtime turns a pattern into the list of the output node
    updateDebugScene(group, { 1: { type: 'pattern', data: wrap(mesh) } });
    expect(meshes(group)).toHaveLength(1);

    updateDebugScene(group, { 1: { type: 'pattern', data: Int32Array.of(15, 0, 0) } });
    expect(group.children).toHaveLength(0);
  });
});

// [12, n, ...n × (x0, y0, z0, x1, y1, z1, group)], one polyline per list of points
function lines(polylines: { group: number; points: number[][] }[]) {
  const words: number[] = [];
  const f = new Float32Array(6);
  for (const { group, points } of polylines) {
    for (let i = 1; i < points.length; i++) {
      f.set([...points[i - 1], ...points[i]]);
      words.push(...new Int32Array(f.buffer), group);
    }
  }
  return Int32Array.from([12, words.length / 7, ...words]);
}

function readSegments(data: Int32Array) {
  const f = new Float32Array(data.buffer, data.byteOffset, data.length);
  return Array.from({ length: data[1] }, (_, i) => [...f.subarray(2 + i * 7, 8 + i * 7)]);
}

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
      updateDebugScene(group, { 1: { type: 'path', data, amount } });
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

  it('applies to instances, points, meshes and lines', () => {
    const group = new Group();
    updateDebugScene(group, { 1: { type: 'path', data: wrap(instances(400)), amount: 0.1 } });
    expect(collect(group).arrows.length).toBeGreaterThan(10);
    expect(collect(group).arrows.length).toBeLessThan(80);

    const pointData = concatEncodedArrays(
      Array.from({ length: 200 }, (_, i) => {
        const p = point([0, i, 0], [1, 0, 0]);
        p[3] = i;
        return wrap(p);
      })
    );
    updateDebugScene(group, { 1: { type: 'points', data: pointData, amount: 0.1 } });
    expect(collect(group).arrows.length).toBeLessThan(60);

    const polylines = Array.from({ length: 200 }, (_, i) => ({
      group: 1,
      points: [[i, 0, 0], [i, 1, 0], [i, 2, 0]]
    }));
    const shown = readSegments(selectLines(lines(polylines), 0.2));
    // both segments of a path stay together
    expect(shown.length % 2).toBe(0);
    expect(shown.length / 2).toBeGreaterThan(20);
    expect(shown.length / 2).toBeLessThan(60);
  });

  it('draws a share of the faces of a mesh', () => {
    const faces = 300;
    const mesh = new Int32Array(4 + faces * 3 + 3 * 7);
    mesh.set([14, 3, faces, 0]);
    for (let f = 0; f < faces; f++) mesh.set([0, 1, 2], 4 + f * 3);
    const drawn = (amount: number) => {
      const group = new Group();
      updateDebugScene(group, { 1: { type: 'mesh', data: mesh, amount } });
      const [m] = group.children.flatMap((g) => g.children.flatMap((p) => p.children as Mesh[]));
      return m.geometry.index!.count / 3;
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
    updateDebugScene(group, { 1: { type: 'path', data, clip: 0.5 } });
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

  it('cuts every path of Myzel lines', () => {
    const data = lines([
      { group: 1, points: [[0, 0, 0], [0, 1, 0], [0, 2, 0]] },
      { group: 2, points: [[5, 0, 0], [5, 4, 0]] }
    ]);
    expect(readSegments(selectLines(data, 1, 0.25))).toEqual([
      [0, 0, 0, 0, 0.5, 0],
      [5, 0, 0, 5, 1, 0]
    ]);
    expect(readSegments(selectLines(data, 1, 0.75))).toEqual([
      [0, 0, 0, 0, 1, 0],
      [0, 1, 0, 0, 1.5, 0],
      [5, 0, 0, 5, 3, 0]
    ]);
  });
});

describe('the defaults of the debug node', () => {
  it('draw everything as it was', () => {
    const data = concatEncodedArrays([
      ...pathItems(30),
      wrap(instances(5)),
      wrap(point([0, 1, 0], [1, 0, 0])),
      wrap(lines([{ group: 1, points: [[0, 0, 0], [0, 1, 0]] }]))
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
    updateDebugScene(before, { 1: { type: 'path', data } });
    const after = new Group();
    updateDebugScene(after, { 1: { type: 'path', data, amount: 1, clip: 1 } });
    expect(positions(after)).toEqual(positions(before));
    expect(positions(before).length).toBeGreaterThan(30);

    const myzel = lines([{ group: 1, points: [[0, 0, 0], [0, 1, 0]] }]);
    expect(selectLines(myzel, 1, 1)).toBe(myzel);
  });
});
