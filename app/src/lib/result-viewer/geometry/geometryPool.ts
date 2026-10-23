import { readGeometryPathId, readInstancePathIds } from '@nodarium/utils';
import {
  Box3,
  BufferAttribute,
  BufferGeometry,
  Float32BufferAttribute,
  Group,
  InstancedMesh,
  Material,
  Matrix4,
  Mesh,
  Sphere
} from 'three';

/** A hash over every value of `arr`, two 32 bit lanes joined into one number */
function fastArrayHash(arr: Int32Array) {
  let a = 0x811c9dc5;
  let b = 0x9e3779b9;
  for (let i = 0; i < arr.length; i++) {
    const v = arr[i];
    a = Math.imul(a ^ v, 0x01000193);
    b = Math.imul(b + v, 0x85ebca6b) ^ (b >>> 15);
  }
  return (a >>> 0) + (b & 0x1fffff) * 0x100000000;
}

type PoolStats = {
  totalVertices: number;
  totalFaces: number;
  /** the bounds of everything shown, in the space of the pool's group */
  bounds: Box3;
};

type Slot<T> = { object: T; key: string };

/**
 * Keeps one object per input and reuses them from update to update. Inputs are matched to objects
 * by key, so removing one input leaves all others on the object they had and only that one is
 * hidden. Objects that are not needed are hidden, not removed.
 */
function createSlotPool<T extends Mesh | InstancedMesh>(
  parentScene: Group,
  options: {
    /** inputs with the same key are matched to the same object */
    key: (data: Int32Array) => string;
    /** brings `existing` (or a new object) to the state of `data`, it may return a replacement */
    fill: (existing: T | null, data: Int32Array, stats: PoolStats) => T;
  }
) {
  const scene = new Group();
  parentScene.add(scene);

  const slots: Slot<T>[] = [];
  const stats: PoolStats = { totalVertices: 0, totalFaces: 0, bounds: new Box3() };

  function update(inputs: Int32Array[]) {
    stats.totalVertices = 0;
    stats.totalFaces = 0;
    stats.bounds.makeEmpty();

    // equal keys are told apart by how often they came up before
    const occurrences = new Map<string, number>();
    const keys = inputs.map((input) => {
      const base = options.key(input);
      const n = occurrences.get(base) ?? 0;
      occurrences.set(base, n + 1);
      return `${base}#${n}`;
    });

    const byKey = new Map<string, Slot<T>>();
    for (const slot of slots) byKey.set(slot.key, slot);

    const used = new Set<Slot<T>>();
    const assigned: (Slot<T> | undefined)[] = keys.map((key) => {
      const slot = byKey.get(key);
      if (slot) used.add(slot);
      return slot;
    });

    // what found no object with its key takes one nobody asked for, or gets a new one
    const spare = slots.filter((slot) => !used.has(slot));
    let nextSpare = 0;

    for (let i = 0; i < inputs.length; i++) {
      let slot = assigned[i];
      if (!slot) {
        slot = spare[nextSpare++];
        if (slot) slot.key = keys[i];
      }

      const existing = slot?.object ?? null;
      const object = options.fill(existing, inputs[i], stats);
      object.visible = true;

      if (!slot) {
        slot = { object, key: keys[i] };
        slots.push(slot);
        scene.add(object);
      } else if (object !== existing) {
        scene.remove(existing!);
        scene.add(object);
        slot.object = object;
      }
      used.add(slot);
    }

    for (const slot of slots) {
      if (!used.has(slot)) slot.object.visible = false;
    }

    return stats;
  }

  return {
    update,
    forEach(callback: (object: T) => void) {
      for (const slot of slots) callback(slot.object);
    }
  };
}

/** Writes `array` into the attribute, a new one is only made when the size changed */
function writeAttribute(geometry: BufferGeometry, name: string, array: Float32Array) {
  const attribute = geometry.getAttribute(name) as BufferAttribute | undefined;
  if (attribute && attribute.array.length === array.length) {
    attribute.array.set(array);
    attribute.needsUpdate = true;
  } else {
    geometry.setAttribute(name, new Float32BufferAttribute(array, 3));
  }
}

function writeIndex(geometry: BufferGeometry, indices: Int32Array, vertexCount: number) {
  const Type = vertexCount > 65535 ? Uint32Array : Uint16Array;
  const index = geometry.getIndex();
  if (index && index.array instanceof Type && index.array.length === indices.length) {
    index.array.set(indices);
    index.needsUpdate = true;
  } else {
    geometry.setIndex(new BufferAttribute(new Type(indices), 1));
  }
}

function setBounds(geometry: BufferGeometry, positions: Float32Array) {
  geometry.boundingBox = new Box3().setFromArray(positions);
  geometry.boundingSphere = geometry.boundingBox.getBoundingSphere(new Sphere());
  return geometry.boundingBox;
}

export function createGeometryPool(parentScene: Group, material: Material) {
  const pool = createSlotPool<Mesh>(parentScene, {
    // the id of the path the geometry was made from
    key: (data) => String(readGeometryPathId(data)),
    fill: (existing, data, stats) => {
      const mesh = existing ?? new Mesh(new BufferGeometry(), material);
      const geometry = mesh.geometry;

      // type, vertex and face amount and the path id, then faces, positions and normals
      const vertexCount = data[1];
      const faceCount = data[2];
      stats.totalVertices += vertexCount;
      stats.totalFaces += faceCount;
      mesh.userData.pathId = readGeometryPathId(data);

      const hash = fastArrayHash(data);
      if (geometry.userData.hash !== hash) {
        const facesStart = 4;
        const positionsStart = facesStart + faceCount * 3;
        const normalsStart = positionsStart + vertexCount * 3;
        const floats = (start: number) =>
          new Float32Array(data.buffer, data.byteOffset + start * 4, vertexCount * 3);

        writeIndex(geometry, data.subarray(facesStart, positionsStart), vertexCount);
        writeAttribute(geometry, 'position', floats(positionsStart));
        writeAttribute(geometry, 'normal', floats(normalsStart));
        // the renderer would compute the bounds from the vertices again, they are known here
        setBounds(geometry, floats(positionsStart));
        geometry.userData = { vertexCount, faceCount, hash };
      }

      stats.bounds.union(geometry.boundingBox!);
      return mesh;
    }
  });

  return {
    setMaterial(newMaterial: Material) {
      material = newMaterial;
      pool.forEach((mesh) => mesh.material = material);
    },
    update: pool.update
  };
}

export function createInstancedGeometryPool(
  parentScene: Group,
  material: Material
) {
  const matrix = new Matrix4();
  const instanceBounds = new Box3();

  const pool = createSlotPool<InstancedMesh>(parentScene, {
    // instances of the same shape and depth share an object
    key: (data) => `${data[1]}:${data[2]}:${data[4]}`,
    fill: (existing, data, stats) => {
      // type, vertex, face and instance amount and the stem depth, then faces, positions, normals
      // and a 4x4 matrix per instance
      const vertexCount = data[1];
      const faceCount = data[2];
      const instanceCount = data[3];
      stats.totalVertices += vertexCount * instanceCount;
      stats.totalFaces += faceCount * instanceCount;

      const facesStart = 5;
      const positionsStart = facesStart + faceCount * 3;
      const normalsStart = positionsStart + vertexCount * 3;
      const matricesStart = normalsStart + vertexCount * 3;
      const floats = (start: number, length: number) =>
        new Float32Array(data.buffer, data.byteOffset + start * 4, length);

      let mesh = existing;
      const geometry = mesh?.geometry ?? new BufferGeometry();

      writeIndex(geometry, data.subarray(facesStart, positionsStart), vertexCount);
      writeAttribute(geometry, 'position', floats(positionsStart, vertexCount * 3));
      writeAttribute(geometry, 'normal', floats(normalsStart, vertexCount * 3));
      const geometryBounds = setBounds(geometry, floats(positionsStart, vertexCount * 3));
      geometry.userData = { vertexCount, faceCount };

      // the matrix buffer can't grow, so the mesh is replaced by a bigger one
      if (mesh && instanceCount > mesh.instanceMatrix.count) {
        mesh.dispose();
        mesh = null;
      }
      mesh ??= new InstancedMesh(geometry, material, instanceCount);
      mesh.count = instanceCount;

      const matrices = floats(matricesStart, instanceCount * 16);
      mesh.instanceMatrix.array.set(matrices);
      mesh.instanceMatrix.needsUpdate = true;

      // which path each of the instances sits on
      mesh.userData.pathIds = Int32Array.from(readInstancePathIds(data));

      // the frustum culling needs the bounds of the geometry moved by all of its instances
      const bounds = mesh.boundingBox ?? new Box3();
      bounds.makeEmpty();
      for (let i = 0; i < instanceCount; i++) {
        matrix.fromArray(matrices, i * 16);
        bounds.union(instanceBounds.copy(geometryBounds).applyMatrix4(matrix));
      }
      mesh.boundingBox = bounds;
      mesh.boundingSphere = bounds.getBoundingSphere(mesh.boundingSphere ?? new Sphere());

      stats.bounds.union(bounds);
      return mesh;
    }
  });

  return {
    setMaterial(newMaterial: Material) {
      material = newMaterial;
      pool.forEach((instance) => instance.material = material);
    },
    update: pool.update
  };
}
