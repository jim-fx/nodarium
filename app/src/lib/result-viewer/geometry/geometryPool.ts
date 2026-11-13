import { readGeometry, readInstances, readMesh, sortResultItems } from '@nodarium/runtime';
import { readGeometryPathId, readInstancePathIds } from '@nodarium/utils';
import {
  Box3,
  BufferAttribute,
  BufferGeometry,
  Color,
  Float32BufferAttribute,
  Group,
  InstancedBufferAttribute,
  InstancedMesh,
  Material,
  Matrix4,
  Mesh,
  MeshMatcapMaterial,
  Sphere,
  SRGBColorSpace
} from 'three';
import { createLinesPool } from './lines';

/**
 * Path ids are hashes that would lose bits as floats in the group attribute, any 24 of them still
 * tell the paths apart
 */
const GROUP_MASK = 0xff_ffff;

function groupAttribute(groups: Int32Array) {
  const out = new Float32Array(groups.length);
  for (let i = 0; i < groups.length; i++) out[i] = groups[i] & GROUP_MASK;
  return out;
}

const coloredMaterials = new WeakMap<Material, Material>();

/** The material where a vertex has no color of its own (alpha 0), its color where it has one */
const VERTEX_COLOR_FRAGMENT = /* glsl */ `
  #if defined( USE_COLOR_ALPHA )
    diffuseColor.rgb = mix(diffuseColor.rgb, diffuseColor.rgb * vColor.rgb, vColor.a);
  #elif defined( USE_COLOR )
    diffuseColor.rgb *= vColor.rgb;
  #endif
`;

/**
 * A color's own light. The ambient light of the viewer adds to the light of the environment map,
 * the matcap of the plant material darkens that again, but a color leaves the matcap out (see
 * the plant material) and would get about one and a half times the light: a yellow-orange
 * turned out a pale yellow. Without the ambient light a surface facing the sky gets about the
 * light of the environment map, a color looks like its value there.
 */
const COLOR_LIGHT_FRAGMENT = /* glsl */ `
  #if defined( USE_COLOR_ALPHA )
    irradiance *= 1.0 - vColor.a;
  #endif
  #include <lights_fragment_end>
`;

/**
 * The AgX tone mapping of the viewer makes bright colors paler and shifts orange towards yellow,
 * the neutral tone mapping keeps a color as it is up to its highlights. Vertices without a color
 * keep the tone mapping of the viewer.
 */
const COLOR_TONE_MAPPING_FRAGMENT = /* glsl */ `
  #if defined( TONE_MAPPING ) && defined( USE_COLOR_ALPHA )
    gl_FragColor.rgb = mix(
      toneMapping(gl_FragColor.rgb),
      NeutralToneMapping(gl_FragColor.rgb),
      vColor.a
    );
  #else
    #include <tonemapping_fragment>
  #endif
`;

/** The matcap where a vertex has no color, its color shaded by the facing where it has one */
const MATCAP_OUTGOING = /* glsl */ `
  vec3 colorShade = vColor.rgb * mix(0.2, 0.8, uv.y);
  vec3 outgoingLight = diffuseColor.rgb * mix(matcapColor.rgb, colorShade, vColor.a);
`;

/**
 * Lets meshes with colors of their own show them with `material`. Only the plant material of the
 * viewer and the thumbnails take colors, the other materials of the viewer show the shape, its
 * normals or its faces and ignore them.
 */
export function takeVertexColors<T extends Material>(material: T) {
  material.userData.vertexColors = true;
  return material;
}

/**
 * The material to show meshes that bring their own colors with, the material as it is unless it
 * takes colors (see takeVertexColors). Vertices without a color of their own (alpha 0, `NO_COLOR`
 * of myzel_core) keep the look of the material, the matcap tint is left out where they have one.
 * Lit materials light and tone map a color so it looks close to its value.
 */
export function withVertexColors(material: Material) {
  if (!material.userData.vertexColors) return material;
  let colored = coloredMaterials.get(material);
  if (colored) return colored;
  colored = material.clone();
  colored.vertexColors = true;
  const isMatcap = material instanceof MeshMatcapMaterial;
  colored.onBeforeCompile = (shader, renderer) => {
    material.onBeforeCompile(shader, renderer);
    shader.fragmentShader = isMatcap
      ? shader.fragmentShader
        .replace('#include <color_fragment>', '')
        .replace('vec3 outgoingLight = diffuseColor.rgb * matcapColor.rgb;', MATCAP_OUTGOING)
      : shader.fragmentShader
        .replace('#include <color_fragment>', VERTEX_COLOR_FRAGMENT)
        .replace('#include <lights_fragment_end>', COLOR_LIGHT_FRAGMENT)
        .replace('#include <tonemapping_fragment>', COLOR_TONE_MAPPING_FRAGMENT);
  };
  colored.customProgramCacheKey = () => `${material.customProgramCacheKey()}:vertex-colors`;
  coloredMaterials.set(material, colored);
  return colored;
}

// only ever called with Int32Arrays, so the loop stays monomorphic and fast
function sameBits(a: Int32Array, b: Int32Array) {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) {
    if (a[i] !== b[i]) return false;
  }
  return true;
}

function sameIndices(a: Uint16Array, b: Int32Array) {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) {
    if (a[i] !== b[i]) return false;
  }
  return true;
}

function bits(array: Float32Array | Uint32Array | Int32Array) {
  return new Int32Array(array.buffer, array.byteOffset, array.length);
}

function sameFloats(attribute: BufferAttribute | undefined, values: Float32Array) {
  return !!attribute && sameBits(bits(attribute.array as Float32Array), bits(values));
}

const color = new Color();

/** Vertex colors are stored in sRGB, the renderer works with linear colors. Alpha stays as it is. */
function linearColors(colors: Uint8Array) {
  const out = new Float32Array(colors.length);
  for (let i = 0; i < colors.length; i += 4) {
    color.setRGB(colors[i] / 255, colors[i + 1] / 255, colors[i + 2] / 255, SRGBColorSpace);
    out[i] = color.r;
    out[i + 1] = color.g;
    out[i + 2] = color.b;
    out[i + 3] = colors[i + 3] / 255;
  }
  return out;
}

/**
 * Whether the geometry shows this data already, so the upload can be skipped. Changed data
 * mostly differs early on, unlike a hash this stops at the first difference.
 */
function showsData(
  geometry: BufferGeometry,
  faces: Int32Array,
  positions: Float32Array,
  normals: Float32Array
) {
  const index = geometry.getIndex()?.array;
  return sameFloats(geometry.getAttribute('position') as BufferAttribute, positions)
    && (index instanceof Uint16Array
      ? sameIndices(index, faces)
      : index instanceof Uint32Array && sameBits(bits(index), faces))
    && sameFloats(geometry.getAttribute('normal') as BufferAttribute, normals);
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
function writeAttribute(
  geometry: BufferGeometry,
  name: string,
  array: Float32Array,
  itemSize = 3
) {
  const attribute = geometry.getAttribute(name) as BufferAttribute | undefined;
  if (attribute && attribute.array.length === array.length) {
    attribute.array.set(array);
    attribute.needsUpdate = true;
  } else {
    geometry.setAttribute(name, new Float32BufferAttribute(array, itemSize));
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

export function createGeometryPool(
  parentScene: Group,
  material: Material,
  layout: 'geometry' | 'mesh' = 'geometry'
) {
  const read = layout === 'mesh' ? readMesh : readGeometry;
  // meshes that bring no colors of their own are shown with the material as it is
  const materialFor = (geometry: BufferGeometry) =>
    geometry.getAttribute('color') ? withVertexColors(material) : material;

  const pool = createSlotPool<Mesh>(parentScene, {
    // the id of the path the geometry was made from, meshes have none and keep their order
    key: (data) => layout === 'mesh' ? 'mesh' : String(readGeometryPathId(data)),
    fill: (existing, data, stats) => {
      const mesh = existing ?? new Mesh(new BufferGeometry(), material);
      const geometry = mesh.geometry;

      const {
        vertexCount,
        faceCount,
        faces,
        positions,
        normals,
        colors,
        groups: meshGroups
      } = read(data);
      stats.totalVertices += vertexCount;
      stats.totalFaces += faceCount;
      if (layout === 'geometry') mesh.userData.pathId = readGeometryPathId(data);
      else mesh.userData.myzel = true;
      // plant geometry is grouped by the path it was made from
      const groups = meshGroups
        ?? new Int32Array(vertexCount).fill(readGeometryPathId(data));

      // one word per vertex, compared like the other attributes
      const colorWords = colors && new Int32Array(colors.buffer, colors.byteOffset, vertexCount);
      const shownColors: Int32Array | undefined = geometry.userData.colors;
      const colorsShown = colorWords
        ? !!shownColors && sameBits(shownColors, colorWords)
        : !geometry.getAttribute('color');
      const shownGroups: Int32Array | undefined = geometry.userData.groups;
      const groupsShown = !!shownGroups && sameBits(shownGroups, groups);

      if (!colorsShown || !groupsShown || !showsData(geometry, faces, positions, normals)) {
        writeIndex(geometry, faces, vertexCount);
        writeAttribute(geometry, 'position', positions);
        writeAttribute(geometry, 'normal', normals);
        if (colors) writeAttribute(geometry, 'color', linearColors(colors), 4);
        else geometry.deleteAttribute('color');
        writeAttribute(geometry, 'group', groupAttribute(groups), 1);
        // the renderer would compute the bounds from the vertices again, they are known here
        setBounds(geometry, positions);
        geometry.userData = {
          vertexCount,
          faceCount,
          colors: colorWords?.slice(),
          groups: groups.slice()
        };
      }
      mesh.material = materialFor(geometry);

      stats.bounds.union(geometry.boundingBox!);
      return mesh;
    }
  });

  return {
    setMaterial(newMaterial: Material) {
      material = newMaterial;
      pool.forEach((mesh) => mesh.material = materialFor(mesh.geometry));
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
      const { vertexCount, faceCount, instanceCount, faces, positions, normals, matrices } =
        readInstances(data);
      stats.totalVertices += vertexCount * instanceCount;
      stats.totalFaces += faceCount * instanceCount;

      let mesh = existing;
      const geometry = mesh?.geometry ?? new BufferGeometry();

      writeIndex(geometry, faces, vertexCount);
      writeAttribute(geometry, 'position', positions);
      writeAttribute(geometry, 'normal', normals);
      const geometryBounds = setBounds(geometry, positions);
      geometry.userData = { vertexCount, faceCount };

      // the matrix buffer can't grow, so the mesh is replaced by a bigger one
      if (mesh && instanceCount > mesh.instanceMatrix.count) {
        mesh.dispose();
        mesh = null;
      }
      mesh ??= new InstancedMesh(geometry, material, instanceCount);
      mesh.count = instanceCount;

      mesh.instanceMatrix.array.set(matrices);
      mesh.instanceMatrix.needsUpdate = true;

      // which path each of the instances sits on, missing ids stay 0 here unlike in readInstances
      const pathIds = Int32Array.from(readInstancePathIds(data));
      mesh.userData.pathIds = pathIds;
      // one group per instance even when the ids are missing, a short buffer would fail the draw
      const groups = new Float32Array(instanceCount);
      groups.set(groupAttribute(pathIds).subarray(0, instanceCount));
      const shownGroups = geometry.getAttribute('instanceGroup') as
        | InstancedBufferAttribute
        | undefined;
      if (shownGroups && shownGroups.array.length === groups.length) {
        shownGroups.array.set(groups);
        shownGroups.needsUpdate = true;
      } else {
        geometry.setAttribute('instanceGroup', new InstancedBufferAttribute(groups, 1));
      }

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

/**
 * Shows a whole result: Plantarium geometry and instances, Myzel meshes, some of which bring their
 * own colors, and Myzel paths as lines. Items of other kinds are skipped.
 */
export function createResultPools(parentScene: Group, material: Material) {
  const geometries = createGeometryPool(parentScene, material);
  const instances = createInstancedGeometryPool(parentScene, material);
  const meshes = createGeometryPool(parentScene, material, 'mesh');
  const lines = createLinesPool(parentScene);

  return {
    setMaterial(newMaterial: Material) {
      geometries.setMaterial(newMaterial);
      instances.setMaterial(newMaterial);
      meshes.setMaterial(newMaterial);
    },
    update(items: Int32Array[]) {
      const sorted = sortResultItems(items);
      const stats = [
        geometries.update(sorted.geometries),
        instances.update(sorted.instances),
        meshes.update(sorted.meshes),
        lines.update(sorted.lines)
      ];
      const bounds = new Box3();
      for (const s of stats) bounds.union(s.bounds);
      return {
        totalVertices: stats.reduce((sum, s) => sum + s.totalVertices, 0),
        totalFaces: stats.reduce((sum, s) => sum + s.totalFaces, 0),
        bounds
      };
    }
  };
}
