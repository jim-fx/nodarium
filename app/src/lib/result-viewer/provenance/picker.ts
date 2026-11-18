import { groupModes, MAX_GROUPS } from './highlight';

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
  kind: 'geometry' | 'instances' | 'mesh';
};

/** What is under a pixel: the object and its instance, or its vertex group for a Myzel mesh */
export type Pick = Pickable & { index: number };

/**
 * `integer` writes the ids into an RGBA32UI target, `bytes` packs them into RGBA8 with 16 bits
 * for the object and 16 for the instance, for browsers that can't read integer targets back
 */
export type PickFormat = 'integer' | 'bytes';

export type PickStats = {
  format: PickFormat;
  picks: number;
  /** ms on the main thread: swapping materials, the draw and starting the readback */
  lastSync: number;
  /** ms until the pixel arrived */
  lastTotal: number;
  /** draw calls of the last id pass */
  lastCalls: number;
  syncSamples: number[];
  totalSamples: number[];
  /** the part of `syncSamples` that the draw took */
  drawSamples: number[];
};

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
  if (kind === 'mesh') defines.PICK_GROUPS = '';
  if (format === 'bytes') defines.PICK_BYTES = '';
  return new RawShaderMaterial({
    glslVersion: GLSL3,
    vertexShader: VERTEX,
    fragmentShader: FRAGMENT,
    side: DoubleSide,
    defines,
    uniforms: { uSlot: { value: 0 }, uGroupModes: groupModes }
  });
}

/** Only the objects of a pick are on this layer, so the id pass draws nothing else of the scene */
const PICK_LAYER = 7;

/** The slot of an object is written into the uniform right before it is drawn */
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
  id.uniformsNeedUpdate = true;
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
    mesh: idMaterial('mesh', format)
  };
  const pickCamera = new PerspectiveCamera();
  const pixel = format === 'integer' ? new Uint32Array(4) : new Uint8Array(4);
  const clearColor = new Color();

  const stats: PickStats = {
    format,
    picks: 0,
    lastSync: 0,
    lastTotal: 0,
    lastCalls: 0,
    syncSamples: [],
    totalSamples: [],
    drawSamples: []
  };

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
    const calls = renderer.info.render.calls;
    const autoReset = renderer.info.autoReset;
    renderer.info.autoReset = false;
    try {
      const startedAt = performance.now();
      renderer.setRenderTarget(target);
      renderer.setClearColor(0x000000, 0);
      renderer.clear();
      renderer.render(scene, pickCamera);
      stats.lastCalls = renderer.info.render.calls - calls;
      const draw = performance.now() - startedAt;
      return { draw, read: renderer.readRenderTargetPixelsAsync(target, 0, 0, 1, 1, pixel) };
    } finally {
      renderer.info.autoReset = autoReset;
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
      const startedAt = performance.now();
      const { draw, read } = drawIds(scene, camera, pickables, x, y, width, height);
      const sync = performance.now() - startedAt;
      await read;
      record(sync, performance.now() - startedAt, draw);
    } finally {
      busy = false;
    }

    const slot = format === 'integer' ? pixel[0] : pixel[0] | (pixel[1] << 8);
    const index = format === 'integer' ? pixel[1] : pixel[2] | (pixel[3] << 8);
    const hit = pickables[slot - 1];
    return hit && { ...hit, index };
  }

  function record(sync: number, total: number, draw: number) {
    stats.picks++;
    stats.lastSync = sync;
    stats.lastTotal = total;
    stats.syncSamples.push(sync);
    stats.totalSamples.push(total);
    stats.drawSamples.push(draw);
    if (stats.syncSamples.length > 500) {
      stats.syncSamples.shift();
      stats.totalSamples.shift();
      stats.drawSamples.shift();
    }
  }

  return {
    pick,
    stats,
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
