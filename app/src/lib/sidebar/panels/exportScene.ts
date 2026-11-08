import { TISSUES } from '#lib/result-viewer/geometry/hyphae.ts';
import {
  BufferAttribute,
  BufferGeometry,
  Color,
  Group,
  InstancedMesh,
  type Material,
  Matrix4,
  Mesh,
  type Object3D
} from 'three';

function groupName(mesh: Mesh, group: number) {
  return mesh.userData.tissueGroups ? (TISSUES[group] ?? `group ${group}`) : `group ${group}`;
}

/**
 * One mesh per material group, each with its own named material, so other programs can assign
 * materials to them. Faces go to the group of their first vertex. Returns null for a single group.
 */
export function splitByGroup(mesh: Mesh): Group | null {
  const source = mesh.geometry;
  const groups = source.getAttribute('group');
  const index = source.getIndex();
  if (!groups || !index || Array.isArray(mesh.material)) return null;

  const facesOf = new Map<number, number[]>();
  for (let i = 0; i < index.count; i += 3) {
    const group = groups.getX(index.getX(i));
    let faces = facesOf.get(group);
    if (!faces) facesOf.set(group, faces = []);
    faces.push(index.getX(i), index.getX(i + 1), index.getX(i + 2));
  }
  if (facesOf.size < 2) return null;

  const result = new Group();
  for (const [group, faces] of [...facesOf].sort(([a], [b]) => a - b)) {
    const remap = new Map<number, number>();
    const vertices: number[] = [];
    const indices = faces.map((v) => {
      let i = remap.get(v);
      if (i === undefined) {
        i = vertices.length;
        remap.set(v, i);
        vertices.push(v);
      }
      return i;
    });
    const geometry = new BufferGeometry();
    for (const [name, attribute] of Object.entries(source.attributes)) {
      const { itemSize } = attribute;
      const array = new Float32Array(vertices.length * itemSize);
      vertices.forEach((v, i) => {
        for (let c = 0; c < itemSize; c++) array[i * itemSize + c] = attribute.getComponent(v, c);
      });
      geometry.setAttribute(name, new BufferAttribute(array, itemSize));
    }
    geometry.setIndex(indices);
    const name = groupName(mesh, group);
    const material: Material = mesh.material.clone();
    material.name = name;
    const part = new Mesh(geometry, material);
    part.name = name;
    result.add(part);
  }
  return result;
}

/**
 * Adds the vertex colors of the meshes to the `v` lines of an OBJ that three's OBJExporter wrote
 * from `scene`, as `v x y z r g b` in sRGB, which Blender and MeshLab read. The exporter writes
 * colors only for points. A vertex without a color of its own (alpha 0) is white.
 */
export function addObjVertexColors(obj: string, scene: Object3D) {
  // the exporter visits the scene in this order and starts every object with an `o` line
  const objects: (BufferAttribute | undefined)[] = [];
  scene.traverse((object) => {
    const kind = object as Partial<Record<'isMesh' | 'isLine' | 'isPoints', boolean>>;
    if (kind.isMesh) {
      const colors = (object as Mesh).geometry.getAttribute('color');
      objects.push(colors instanceof BufferAttribute ? colors : undefined);
    } else if (kind.isLine || kind.isPoints) {
      objects.push(undefined);
    }
  });
  if (!objects.some(Boolean)) return obj;

  const color = new Color();
  let object = -1;
  let vertex = 0;
  return obj.split('\n').map((line) => {
    if (line.startsWith('o ')) {
      object++;
      vertex = 0;
      return line;
    }
    const colors = objects[object];
    if (!colors || !line.startsWith('v ')) return line;
    const i = vertex++;
    const alpha = colors.itemSize === 4 ? colors.getW(i) : 1;
    color.setRGB(colors.getX(i), colors.getY(i), colors.getZ(i)).lerp(white, 1 - alpha);
    const { r, g, b } = color.convertLinearToSRGB();
    return `${line} ${round(r)} ${round(g)} ${round(b)}`;
  }).join('\n');
}

const white = new Color(1, 1, 1);
const round = (v: number) => Math.round(v * 10000) / 10000;

/**
 * Builds a copy of the scene that is safe to hand to the exporters:
 * hidden objects (the geometry pools keep stale meshes around) are dropped
 * and instanced meshes are expanded to one plain mesh per instance, since
 * the OBJ exporter ignores the instance matrices. Meshes with several
 * material groups are split into one named mesh per group.
 */
export function createExportScene(source: Object3D): Group {
  const result = new Group();
  const matrix = new Matrix4();

  function visit(object: Object3D, parent: Object3D) {
    if (!object.visible) return;

    if (object instanceof InstancedMesh) {
      for (let i = 0; i < object.count; i++) {
        object.getMatrixAt(i, matrix);
        const mesh = new Mesh(object.geometry, object.material);
        matrix.decompose(mesh.position, mesh.quaternion, mesh.scale);
        parent.add(mesh);
      }
      return;
    }

    let copy: Object3D;
    if (object instanceof Mesh) {
      copy = splitByGroup(object) ?? new Mesh(object.geometry, object.material);
    } else {
      copy = new Group();
    }
    copy.name = object.name;
    copy.position.copy(object.position);
    copy.quaternion.copy(object.quaternion);
    copy.scale.copy(object.scale);
    parent.add(copy);

    for (const child of object.children) visit(child, copy);
  }

  for (const child of source.children) visit(child, result);
  result.position.copy(source.position);
  result.quaternion.copy(source.quaternion);
  result.scale.copy(source.scale);
  return result;
}
