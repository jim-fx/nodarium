import { createGroupModes, type GroupModes, MAX_GROUPS } from './highlight';

import {
  type Camera,
  Color,
  DoubleSide,
  GLSL3,
  type InstancedMesh,
  type Mesh,
  NearestFilter,
  type Object3D,
  PerspectiveCamera,
  RawShaderMaterial,
  RGBAFormat,
  RGBAIntegerFormat,
  UnsignedByteType,
  UnsignedIntType,
  type WebGLRenderer,
  WebGLRenderTarget
} from 'three';

export type Pickable = {
  object: Mesh | InstancedMesh;
  kind: 'geometry' | 'instances' | 'groups';
};

/** What is under a pixel: the object and its instance, or its vertex group for a geometry with groups */
export type Pick = Pickable & { index: number };

/**
 * `integer` writes the ids into an RGBA32UI target, `bytes` packs them into RGBA8 with 16 bits
 * for the object and 16 for the instance, for browsers that can't read integer targets back
 */
export type PickFormat = 'integer' | 'bytes';

const VERTEX = /* glsl */ `
  precision highp float;
  precision highp int;

  uniform mat4 modelMatrix;
  uniform mat4 viewMatrix;
  uniform mat4 projectionMatrix;

  in vec3 position;
  #ifdef PICK_INSTANCES
    in mat4 instanceMatrix;
    in float instanceHighlight;
  #endif
  #ifdef PICK_GROUPS
    in float group;
    uniform float uGroupModes[${MAX_GROUPS}];
  #endif

  flat out uint vIndex;
  flat out float vHidden;

  void main() {
    vec4 pos = vec4(position, 1.0);
    vIndex = 0u;
    vHidden = 0.0;
    #ifdef PICK_INSTANCES
      pos = instanceMatrix * pos;
      vIndex = uint(gl_InstanceID);
    #endif
    #ifdef PICK_GROUPS
      vIndex = uint(group + 0.5);
      // a tissue the local view hides can't be picked either
      vHidden = uGroupModes[int(clamp(group + 0.5, 0.0, ${MAX_GROUPS - 1}.0))];
    #endif
    gl_Position = projectionMatrix * viewMatrix * modelMatrix * pos;
    #ifdef PICK_INSTANCES
      // an instance the local view hides can't be picked either
      if (instanceHighlight > 4.5) gl_Position = vec4(2.0, 2.0, 2.0, 1.0);
    #endif
  }
`;

const FRAGMENT = /* glsl */ `
  precision highp float;
  precision highp int;

  uniform uint uSlot;
  flat in uint vIndex;
  flat in float vHidden;

  #ifdef PICK_BYTES
    layout(location = 0) out highp vec4 outId;
  #else
    layout(location = 0) out highp uvec4 outId;
  #endif

  void main() {
    if (vHidden > 4.5) discard;
    #ifdef PICK_BYTES
      outId = vec4(
        float(uSlot & 255u), float((uSlot >> 8) & 255u),
        float(vIndex & 255u), float((vIndex >> 8) & 255u)
      ) / 255.0;
    #else
      outId = uvec4(uSlot, vIndex, 0u, 1u);
    #endif
  }
`;

function idMaterial(kind: Pickable['kind'], format: PickFormat) {
  const defines: Record<string, string> = {};
  if (kind === 'instances') defines.PICK_INSTANCES = '';
  if (kind === 'groups') defines.PICK_GROUPS = '';
  if (format === 'bytes') defines.PICK_BYTES = '';
  return new RawShaderMaterial({
    glslVersion: GLSL3,
    vertexShader: VERTEX,
    fragmentShader: FRAGMENT,
    side: DoubleSide,
    defines,
    uniforms: { uSlot: { value: 0 }, uGroupModes: createGroupModes() }
  });
}

/** Only the objects of a pick are on this layer, so the id pass draws nothing else of the scene */
const PICK_LAYER = 7;

/** Every group shown, for a mesh that was never highlighted */
const NO_MODES = createGroupModes();

/**
 * The slot of an object and the modes of its groups are written into the uniforms right before
 * it is drawn
 */
function writeSlot(
  this: Object3D,
  _renderer: unknown,
  _scene: unknown,
  _camera: unknown,
  _geometry: unknown,
  material: unknown
) {
  const id = material as RawShaderMaterial;
  id.uniforms.uSlot.value = this.userData.pickSlot;
  id.uniforms.uGroupModes.value = (this.userData.groupModes as GroupModes | undefined ?? NO_MODES)
    .value;
  id.uniformsNeedUpdate = true;
}

/**
 * The slot (1 for the first pickable, 0 for nothing) and the instance or group a pixel of the id
 * pass holds. `bytes` packs 16 bits of each into the four channels.
 */
export function decodePixel(format: PickFormat, pixel: ArrayLike<number>) {
  return format === 'integer'
    ? { slot: pixel[0], index: pixel[1] }
    : { slot: pixel[0] | (pixel[1] << 8), index: pixel[2] | (pixel[3] << 8) };
}

/**
 * Tells what is under a pixel by drawing the objects once more, into a target of one pixel, each
 * with its number as its colour. The camera only sees that pixel, so nearly every object is culled
 * before it is drawn. Nothing is kept between picks: no BVH, nothing to rebuild with a new result.
 */
export function createPicker(renderer: WebGLRenderer, format: PickFormat = 'integer') {
  const target = new WebGLRenderTarget(1, 1, {
    format: format === 'integer' ? RGBAIntegerFormat : RGBAFormat,
    type: format === 'integer' ? UnsignedIntType : UnsignedByteType,
    minFilter: NearestFilter,
    magFilter: NearestFilter,
    depthBuffer: true,
    generateMipmaps: false
  });
  if (format === 'integer') target.texture.internalFormat = 'RGBA32UI';

  const materials = {
    geometry: idMaterial('geometry', format),
    instances: idMaterial('instances', format),
    groups: idMaterial('groups', format)
  };
  const pickCamera = new PerspectiveCamera();
  const pixel = format === 'integer' ? new Uint32Array(4) : new Uint8Array(4);
  const clearColor = new Color();

  let busy = false;

  /** Draws the ids of what the pixel shows and starts reading it back */
  function drawIds(
    scene: Object3D,
    camera: PerspectiveCamera,
    pickables: Pickable[],
    x: number,
    y: number,
    width: number,
    height: number
  ) {
    const shown: Mesh['material'][] = [];
    for (let i = 0; i < pickables.length; i++) {
      const { object, kind } = pickables[i];
      shown.push(object.material);
      object.material = materials[kind];
      object.userData.pickSlot = i + 1;
      object.onBeforeRender = writeSlot;
      object.layers.enable(PICK_LAYER);
    }

    pickCamera.copy(camera, false);
    pickCamera.layers.set(PICK_LAYER);
    pickCamera.setViewOffset(width, height, x, y, 1, 1);

    const previousTarget = renderer.getRenderTarget();
    const previousAlpha = renderer.getClearAlpha();
    renderer.getClearColor(clearColor);
    try {
      renderer.setRenderTarget(target);
      renderer.setClearColor(0x000000, 0);
      renderer.clear();
      renderer.render(scene, pickCamera);
      return renderer.readRenderTargetPixelsAsync(target, 0, 0, 1, 1, pixel);
    } finally {
      renderer.setRenderTarget(previousTarget);
      renderer.setClearColor(clearColor, previousAlpha);
      for (let i = 0; i < pickables.length; i++) {
        const { object } = pickables[i];
        object.material = shown[i];
        object.onBeforeRender = noop;
        object.layers.disable(PICK_LAYER);
      }
    }
  }

  /**
   * @param x in CSS pixels from the left of the canvas, like `width`
   * @returns undefined when nothing is under the pixel, or while another pick is on its way
   */
  async function pick(
    scene: Object3D,
    camera: Camera,
    pickables: Pickable[],
    x: number,
    y: number,
    width: number,
    height: number
  ): Promise<Pick | undefined> {
    if (busy || !(camera instanceof PerspectiveCamera)) return;
    busy = true;
    try {
      await drawIds(scene, camera, pickables, x, y, width, height);
    } finally {
      busy = false;
    }

    const { slot, index } = decodePixel(format, pixel);
    const hit = pickables[slot - 1];
    return hit && { ...hit, index };
  }

  return {
    pick,
    get busy() {
      return busy;
    },
    dispose() {
      target.dispose();
      for (const material of Object.values(materials)) material.dispose();
    }
  };
}

function noop() {}
