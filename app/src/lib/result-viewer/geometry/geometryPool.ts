import { readGeometry, readInstances, sortResultItems } from '@nodarium/runtime';
import { readGeometryPathId, readInstancePathIds } from '@nodarium/utils';
import {
  Box3,
  BufferAttribute,
  BufferGeometry,
  Color,
  ColorManagement,
  Group,
  InstancedBufferAttribute,
  InstancedMesh,
  Material,
  Matrix4,
  Mesh,
  MeshMatcapMaterial,
  Sphere,
  SRGBColorSpace,
  Vector3
} from 'three';
import {
  createGroupModes,
  type GroupModes,
  HIGHLIGHT,
  type HighlightMode,
  type PartHighlight,
  setGroupModes,
  withGroupHighlight,
  withHighlight
} from '../provenance/highlight';
import type { Pickable } from '../provenance/picker';

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

/**
 * Vertex colors are kept as the sRGB bytes of the result, the renderer works with linear colors.
 * The same conversion as `Color.setRGB(..., SRGBColorSpace)`, alpha stays as it is. Like there,
 * nothing is converted while the color management of three is off.
 */
const SRGB_COLOR_VERTEX = /* glsl */ `
  #include <color_vertex>
  #ifdef USE_COLOR
    vColor.rgb = mix(
      vColor.rgb * 0.0773993808,
      pow(vColor.rgb * 0.9478672986 + 0.0521327014, vec3(2.4)),
      step(vec3(0.04045), vColor.rgb)
    );
  #endif
`;

/** The material where a vertex has no color of its own (alpha 0), its color where it has one */
const VERTEX_COLOR_FRAGMENT = /* glsl */ `
  #if defined( USE_COLOR_ALPHA )
    diffuseColor.rgb = mix(diffuseColor.rgb, diffuseColor.rgb * vColor.rgb, vColor.a);
  #elif defined( USE_COLOR )
    diffuseColor.rgb *= vColor.rgb;
  #endif
`;

/**
 * A color's own light. The ambient and fill lights of the viewer add to the light of the
 * environment map, the matcap of the plant material darkens that again, but a color leaves the
 * matcap out (see the plant material) and would get too much light: a yellow-orange turned out a
 * pale yellow. Without them a color gets the environment map and the key light, and looks close
 * to its value and to the thumbnails.
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
 * Lets `material` read the group of every vertex from the `group` attribute. Only the meshes shown
 * with such a material have the attribute, everybody else saves its 4 bytes per vertex.
 */
export function takeGroups<T extends Material>(material: T) {
  material.userData.groups = true;
  return material;
}

/**
 * The material to show meshes that bring their own colors with, the material as it is unless it
 * takes colors (see takeVertexColors). Vertices without a color of their own (alpha 0, `NO_COLOR`
 * of nodarium_utils) keep the look of the material, the matcap tint is left out where they have one.
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
    if (ColorManagement.enabled) {
      shader.vertexShader = shader.vertexShader.replace(
        '#include <color_vertex>',
        SRGB_COLOR_VERTEX
      );
    }
    shader.fragmentShader = isMatcap
      ? shader.fragmentShader
        .replace('#include <color_fragment>', '')
        .replace('vec3 outgoingLight = diffuseColor.rgb * matcapColor.rgb;', MATCAP_OUTGOING)
      : shader.fragmentShader
        .replace('#include <color_fragment>', VERTEX_COLOR_FRAGMENT)
        .replace('#include <lights_fragment_end>', COLOR_LIGHT_FRAGMENT)
        .replace('#include <tonemapping_fragment>', COLOR_TONE_MAPPING_FRAGMENT);
  };
  colored.customProgramCacheKey = () =>
    `${material.customProgramCacheKey()}:vertex-colors:${ColorManagement.enabled}`;
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

function bits(array: Float32Array | Uint32Array | Int32Array | Uint8Array) {
  return new Int32Array(array.buffer, array.byteOffset, array.byteLength >> 2);
}

function sameFloats(attribute: BufferAttribute | undefined, values: Float32Array) {
  return !!attribute && sameBits(bits(attribute.array as Float32Array), bits(values));
}

/** colors as one word per vertex, as the result has them */
function sameColors(attribute: BufferAttribute | undefined, words: Int32Array) {
  return attribute?.array instanceof Uint8Array && sameBits(bits(attribute.array), words);
}

const color = new Color();

/** The linear colors that the shader makes of the sRGB bytes (see SRGB_COLOR_VERTEX) */
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

type Slot<T> = { object: T; key: string; used: boolean };

/**
 * The plants show and hide paths with every seed, their objects are far smaller than this and stay
 * for reuse. A hidden mushroom would keep tens of MB in the page and on the GPU.
 */
const FREE_HIDDEN_BYTES = 1 << 20;

function objectBytes(object: Mesh | InstancedMesh) {
  const geometry = object.geometry;
  let bytes = geometry.index?.array.byteLength ?? 0;
  for (const attribute of Object.values(geometry.attributes)) {
    bytes += attribute.array.byteLength;
  }
  if ('isInstancedMesh' in object) bytes += object.instanceMatrix.array.byteLength;
  return bytes;
}

/**
 * Keeps one object per input and reuses them from update to update. Inputs are matched to objects
 * by key, so removing one input leaves all others on the object they had and only that one is
 * hidden. Objects that are not needed are hidden for reuse, unless they hold more than
 * FREE_HIDDEN_BYTES: those are removed and their buffers freed.
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
        slot = { object, key: keys[i], used: true };
        slots.push(slot);
        scene.add(object);
      } else if (object !== existing) {
        scene.remove(existing!);
        scene.add(object);
        slot.object = object;
      }
      used.add(slot);
    }

    let kept = 0;
    for (const slot of slots) {
      slot.used = used.has(slot);
      if (slot.used) {
        slots[kept++] = slot;
      } else if (objectBytes(slot.object) > FREE_HIDDEN_BYTES) {
        scene.remove(slot.object);
        slot.object.geometry.dispose();
        slot.object.dispose();
      } else {
        slot.object.visible = false;
        slots[kept++] = slot;
      }
    }
    slots.length = kept;

    return stats;
  }

  return {
    update,
    forEach(callback: (object: T) => void) {
      for (const slot of slots) callback(slot.object);
    },
    /** only the objects that show an input, not the ones kept for reuse */
    forEachUsed(callback: (object: T) => void) {
      for (const slot of slots) if (slot.used) callback(slot.object);
    }
  };
}

/** Writes `array` into the attribute, a new one is only made when the size changed */
function writeAttribute(
  geometry: BufferGeometry,
  name: string,
  array: Float32Array | Uint8Array,
  itemSize = 3,
  normalized = false
) {
  const attribute = geometry.getAttribute(name) as BufferAttribute | undefined;
  if (
    attribute && attribute.array.constructor === array.constructor
    && attribute.array.length === array.length
  ) {
    attribute.array.set(array);
    attribute.needsUpdate = true;
  } else {
    geometry.setAttribute(name, new BufferAttribute(array.slice(), itemSize, normalized));
  }
}

/**
 * The renderer frees the buffers of the attributes a geometry has when it is disposed, an
 * attribute that was replaced or deleted before would keep its buffer until it is collected.
 * The next render uploads the geometry again.
 */
function freeBuffers(geometry: BufferGeometry) {
  geometry.dispose();
}

/** Frees the buffers of a geometry whose arrays are about to be replaced by ones of another size */
function freeBuffersOnResize(geometry: BufferGeometry, vertexCount: number, faceCount: number) {
  const shown = geometry.userData;
  if (shown.vertexCount === undefined) return;
  if (shown.vertexCount !== vertexCount || shown.faceCount !== faceCount) freeBuffers(geometry);
}

function removeAttribute(geometry: BufferGeometry, name: string) {
  if (!geometry.getAttribute(name)) return;
  freeBuffers(geometry);
  geometry.deleteAttribute(name);
}

/** Colors as normalized bytes, four per vertex, converted to linear in the shader */
function writeColors(geometry: BufferGeometry, colors: Uint8Array | undefined) {
  if (colors) writeAttribute(geometry, 'color', colors, 4, true);
  else removeAttribute(geometry, 'color');
}

/**
 * The group attribute of the groups in userData, only while the material shows them or picking
 * tells them apart (`keep`)
 */
function writeGroups(geometry: BufferGeometry, material: Material, changed: boolean, keep = false) {
  const groups: Int32Array | undefined = geometry.userData.groups;
  if ((!material.userData.groups && !keep) || !groups) {
    removeAttribute(geometry, 'group');
  } else if (changed || !geometry.getAttribute('group')) {
    writeAttribute(geometry, 'group', groupAttribute(groups), 1);
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

/** Geometry that brings no colors of its own is shown with the material as it is */
function materialFor(geometry: BufferGeometry, material: Material) {
  return geometry.getAttribute('color') ? withVertexColors(material) : material;
}

/**
 * Geometry: one per path of a plant, highlighted and picked by its path, or a geometry with
 * groups like a mushroom, highlighted and picked by the creators of its groups
 */
export function createGeometryPool(parentScene: Group, material: Material) {
  let keepGroups = false;

  const pool = createSlotPool<Mesh>(parentScene, {
    // the id of the path the geometry was made from, others keep their order
    key: (data) => String(readGeometryPathId(data)),
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
        groups: ownGroups,
        creators,
        pathId
      } = readGeometry(data);
      stats.totalVertices += vertexCount;
      stats.totalFaces += faceCount;
      mesh.userData.pathId = pathId;
      mesh.userData.grouped = !!ownGroups;
      // by group, the graph node that made it: what leads from a tissue of a mushroom to its node
      mesh.userData.creators = creators;
      // a geometry without groups is one, the one of its path
      const groups = ownGroups ?? new Int32Array(vertexCount).fill(pathId);

      const colorAttribute = geometry.getAttribute('color') as BufferAttribute | undefined;
      const colorsShown = colors ? sameColors(colorAttribute, bits(colors)) : !colorAttribute;
      // kept to compare with and to make the attribute of when the material shows the groups
      const shownGroups: Int32Array | undefined = geometry.userData.groups;
      const groupsShown = !!shownGroups && sameBits(shownGroups, groups);

      if (!colorsShown || !groupsShown || !showsData(geometry, faces, positions, normals)) {
        freeBuffersOnResize(geometry, vertexCount, faceCount);
        writeIndex(geometry, faces, vertexCount);
        writeAttribute(geometry, 'position', positions);
        writeAttribute(geometry, 'normal', normals);
        writeColors(geometry, colors);
        // the renderer would compute the bounds from the vertices again, they are known here
        setBounds(geometry, positions);
        geometry.userData = {
          vertexCount,
          faceCount,
          groups: groupsShown ? shownGroups : groups.slice()
        };
      }
      writeGroups(geometry, material, !groupsShown, keepGroups && !!ownGroups);
      mesh.material = materialFor(geometry, material);

      stats.bounds.union(geometry.boundingBox!);
      return mesh;
    }
  });

  function rewriteGroups() {
    pool.forEach((mesh) =>
      writeGroups(mesh.geometry, material, false, keepGroups && mesh.userData.grouped)
    );
  }

  return {
    setMaterial(newMaterial: Material) {
      material = newMaterial;
      rewriteGroups();
      pool.forEach((mesh) => mesh.material = materialFor(mesh.geometry, material));
    },
    /**
     * Keeps the groups of the geometry with groups on the GPU whatever the material, a pick or a
     * hover reads them there
     */
    setKeepGroups(keep: boolean) {
      if (keep === keepGroups) return;
      keepGroups = keep;
      rewriteGroups();
    },
    update: pool.update,
    /**
     * Shows a geometry in the mode of its path, one with groups in the modes of their creators, as
     * it came out of `update` without a highlight
     */
    setHighlight(highlight: PartHighlight | undefined) {
      pool.forEachUsed((mesh) => {
        const grouped: boolean = mesh.userData.grouped;
        const mode = highlight
          ? highlight.paths.get(mesh.userData.pathId) ?? highlight.rest
          : HIGHLIGHT.none;
        const byGroup = grouped && !!highlight && !!mesh.userData.creators
          && !!mesh.geometry.getAttribute('group');
        let groupModes: GroupModes | undefined = mesh.userData.groupModes;
        if (grouped) {
          groupModes ??= mesh.userData.groupModes = createGroupModes();
          setGroupModes(groupModes, mesh.userData.creators, byGroup ? highlight : undefined);
        }
        const shown = materialFor(mesh.geometry, material);
        // by group the geometry stays, the shader leaves out the groups the local view hides
        mesh.material = byGroup && groupModes
          ? withGroupHighlight(shown, groupModes)
          : mode === HIGHLIGHT.none
          ? shown
          : withHighlight(shown, mode);
        mesh.visible = byGroup || mode !== HIGHLIGHT.hidden;
      });
    },
    collectPickables(out: Pickable[]) {
      pool.forEachUsed((object) => {
        if (object.visible) {
          out.push({ object, kind: object.userData.grouped ? 'groups' : 'geometry' });
        }
      });
    },
    /** Grows `bounds` by the geometry whose path is in `pathIds` */
    unionBounds(pathIds: Set<number>, bounds: Box3) {
      pool.forEachUsed((mesh) => {
        if (!mesh.userData.grouped && pathIds.has(mesh.userData.pathId)) {
          bounds.union(mesh.geometry.boundingBox!);
        }
      });
    },
    /** Grows `bounds` by the vertices of the groups that one of `creators` made */
    unionTissueBounds(creators: Set<number>, bounds: Box3) {
      if (!creators.size) return;
      pool.forEachUsed((mesh) => {
        const made = usedGroups(mesh, creators);
        if (!made.size) return;
        const groups: Int32Array = mesh.geometry.userData.groups;
        const positions = mesh.geometry.getAttribute('position').array;
        for (let v = 0; v < groups.length; v++) {
          if (!made.has(groups[v])) continue;
          bounds.expandByPoint(
            point.set(positions[v * 3], positions[v * 3 + 1], positions[v * 3 + 2])
          );
        }
      });
    },
    /** Adds the creators that made at least one vertex of a geometry with groups to `out` */
    collectCreators(out: Set<number>) {
      pool.forEachUsed((mesh) => {
        const creators: Int32Array | undefined = mesh.userData.creators;
        const groups: Int32Array | undefined = mesh.geometry.userData.groups;
        if (!mesh.userData.grouped || !creators || !groups) return;
        const used = new Uint8Array(creators.length);
        for (let v = 0; v < groups.length; v++) used[groups[v]] = 1;
        for (let group = 0; group < creators.length; group++) {
          if (used[group]) out.add(creators[group]);
        }
      });
    }
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
      const {
        vertexCount,
        faceCount,
        instanceCount,
        faces,
        positions,
        normals,
        matrices,
        colors
      } = readInstances(data);
      stats.totalVertices += vertexCount * instanceCount;
      stats.totalFaces += faceCount * instanceCount;

      let mesh = existing;
      const geometry = mesh?.geometry ?? new BufferGeometry();

      freeBuffersOnResize(geometry, vertexCount, faceCount);
      writeIndex(geometry, faces, vertexCount);
      writeAttribute(geometry, 'position', positions);
      writeAttribute(geometry, 'normal', normals);
      // the colors of the shape, every instance shows the same
      writeColors(geometry, colors);
      const geometryBounds = setBounds(geometry, positions);
      geometry.userData = { vertexCount, faceCount };

      // the matrix buffer can't grow, so the mesh is replaced by a bigger one
      if (mesh && instanceCount > mesh.instanceMatrix.count) {
        mesh.dispose();
        mesh = null;
      }
      mesh ??= new InstancedMesh(geometry, material, instanceCount);
      mesh.material = materialFor(geometry, material);
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
      pool.forEach((instance) => instance.material = materialFor(instance.geometry, material));
    },
    update: pool.update,
    /** Shows every instance in the mode of the path it sits on */
    setHighlight(highlight: PartHighlight | undefined) {
      pool.forEachUsed((mesh) => {
        const shown = materialFor(mesh.geometry, material);
        if (!highlight) {
          mesh.material = shown;
          return;
        }
        mesh.material = withHighlight(shown, HIGHLIGHT.none);
        writeInstanceHighlight(mesh, highlight);
      });
    },
    collectPickables(out: Pickable[]) {
      pool.forEachUsed((object) => out.push({ object, kind: 'instances' }));
    },
    /** Grows `bounds` by the instances that sit on a path in `pathIds` */
    unionBounds(pathIds: Set<number>, bounds: Box3) {
      pool.forEachUsed((mesh) => {
        const ids: Int32Array = mesh.userData.pathIds;
        const geometryBounds = mesh.geometry.boundingBox!;
        for (let i = 0; i < mesh.count; i++) {
          if (!pathIds.has(ids[i])) continue;
          matrix.fromArray(mesh.instanceMatrix.array, i * 16);
          bounds.union(instanceBounds.copy(geometryBounds).applyMatrix4(matrix));
        }
      });
    }
  };
}

const point = new Vector3();

/** The groups of a geometry that one of `creators` made */
function usedGroups(mesh: Mesh, creators: Set<number>) {
  const made = new Set<number>();
  const table: Int32Array | undefined = mesh.userData.creators;
  if (!table) return made;
  for (let group = 0; group < table.length; group++) {
    if (creators.has(table[group])) made.add(group);
  }
  return made;
}

/** One mode per instance, the attribute is as long as the matrix buffer so it is only made once */
function writeInstanceHighlight(mesh: InstancedMesh, highlight: PartHighlight) {
  const geometry = mesh.geometry;
  let attribute = geometry.getAttribute('instanceHighlight') as
    | InstancedBufferAttribute
    | undefined;
  if (!attribute || attribute.count !== mesh.instanceMatrix.count) {
    if (attribute) freeBuffers(geometry);
    attribute = new InstancedBufferAttribute(new Float32Array(mesh.instanceMatrix.count), 1);
    geometry.setAttribute('instanceHighlight', attribute);
  }
  const modes = attribute.array as Float32Array;
  const ids: Int32Array = mesh.userData.pathIds;
  let previousId = NaN;
  let mode: HighlightMode = highlight.rest;
  for (let i = 0; i < mesh.count; i++) {
    // the instances of a path follow each other
    if (ids[i] !== previousId) {
      previousId = ids[i];
      mode = highlight.instances.get(previousId) ?? highlight.rest;
    }
    modes[i] = mode;
  }
  attribute.needsUpdate = true;
}

/**
 * The geometry as other programs expect it: linear colors as floats and the groups as an
 * attribute. The viewer keeps sRGB bytes and the groups only for the material that shows them.
 */
export function exportGeometry(geometry: BufferGeometry) {
  const colors = geometry.getAttribute('color') as BufferAttribute | undefined;
  const srgbColors = colors?.array instanceof Uint8Array ? colors.array : undefined;
  const groups: Int32Array | undefined = geometry.getAttribute('group')
    ? undefined
    : geometry.userData.groups;
  if (!srgbColors && !groups) return geometry;

  const copy = new BufferGeometry();
  copy.setIndex(geometry.getIndex());
  for (const [name, attribute] of Object.entries(geometry.attributes)) {
    copy.setAttribute(name, attribute);
  }
  if (srgbColors) copy.setAttribute('color', new BufferAttribute(linearColors(srgbColors), 4));
  if (groups) copy.setAttribute('group', new BufferAttribute(groupAttribute(groups), 1));
  copy.boundingBox = geometry.boundingBox;
  copy.boundingSphere = geometry.boundingSphere;
  return copy;
}

/**
 * Shows a whole result: geometry, some of which brings its own colors and groups, and instances.
 * Items of other kinds are skipped.
 */
export function createResultPools(parentScene: Group, material: Material) {
  const geometries = createGeometryPool(parentScene, material);
  const instances = createInstancedGeometryPool(parentScene, material);

  let highlight: PartHighlight | undefined;
  let picking = false;

  /**
   * The geometry with groups keeps them on the GPU from the first pick or highlight on, 4 bytes
   * per vertex. Until then a result is shown as without the link.
   */
  function keepGroups() {
    if (picking) geometries.setKeepGroups(true);
  }

  function applyHighlight() {
    geometries.setHighlight(highlight);
    instances.setHighlight(highlight);
  }

  return {
    setMaterial(newMaterial: Material) {
      geometries.setMaterial(newMaterial);
      instances.setMaterial(newMaterial);
      if (highlight) applyHighlight();
    },
    /** `undefined` shows the result as it is, and nothing is done for it on the next updates */
    setHighlight(next: PartHighlight | undefined) {
      if (!next && !highlight) return;
      if (next) keepGroups();
      highlight = next;
      applyHighlight();
    },
    /** Whether parts are picked at all, see `keepGroups` */
    setPicking(on: boolean) {
      picking = on;
      if (!on) geometries.setKeepGroups(false);
    },
    /** The objects a pick can hit, in the order of their slots */
    pickables() {
      keepGroups();
      const out: Pickable[] = [];
      geometries.collectPickables(out);
      instances.collectPickables(out);
      return out;
    },
    /**
     * The bounds of the geometry of the paths in `pathIds`, of the instances on `instancePaths`
     * and of the tissues that `creators` made
     */
    boundsOf(paths: Set<number>, instancePaths: Set<number>, creators: Set<number>) {
      const bounds = new Box3();
      geometries.unionBounds(paths, bounds);
      instances.unionBounds(instancePaths, bounds);
      geometries.unionTissueBounds(creators, bounds);
      return bounds;
    },
    /** The graph nodes, by creator id, that made a group of the geometry shown */
    meshCreators() {
      const creators = new Set<number>();
      geometries.collectCreators(creators);
      return creators;
    },
    update(items: Int32Array[]) {
      const sorted = sortResultItems(items);
      const stats = [
        geometries.update(sorted.geometries),
        instances.update(sorted.instances)
      ];
      const bounds = new Box3();
      for (const s of stats) bounds.union(s.bounds);
      // the objects came out of the update with their plain material
      if (highlight) applyHighlight();
      return {
        totalVertices: stats.reduce((sum, s) => sum + s.totalVertices, 0),
        totalFaces: stats.reduce((sum, s) => sum + s.totalFaces, 0),
        bounds
      };
    }
  };
}
