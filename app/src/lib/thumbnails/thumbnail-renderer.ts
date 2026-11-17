import { createResultPools, takeVertexColors } from '#lib/result-viewer/geometry/geometryPool.ts';
import { splitNestedArray } from '@nodarium/utils';
import {
  Box3,
  DoubleSide,
  Group,
  type InstancedMesh,
  type Mesh,
  MeshMatcapMaterial,
  PerspectiveCamera,
  Scene,
  SRGBColorSpace,
  Texture,
  Vector3,
  WebGLRenderer
} from 'three';

const FOV = 50;
const PADDING = 1.08;

/** Where the camera looks from: `default` is the angle of the viewer's own camera */
export type View = 'default' | 'front' | 'side' | 'top';

const VIEWS: Record<View, { direction: Vector3; up: Vector3 }> = {
  default: { direction: new Vector3(1, 1, 1).normalize(), up: new Vector3(0, 1, 0) },
  front: { direction: new Vector3(0, 0, 1), up: new Vector3(0, 1, 0) },
  side: { direction: new Vector3(1, 0, 0), up: new Vector3(0, 1, 0) },
  top: { direction: new Vector3(0, 1, 0), up: new Vector3(0, 0, -1) }
};

const right = new Vector3();
const up = new Vector3();
const corner = new Vector3();

/**
 * Moves the camera along `direction` until all corners of the box are in view,
 * and centers the view on the projected box (tighter than fitting a bounding sphere)
 */
function fitCameraToBox(camera: PerspectiveCamera, box: Box3, direction: Vector3) {
  const tan = Math.tan((FOV / 2) * (Math.PI / 180));
  right.crossVectors(camera.up, direction).normalize();
  up.crossVectors(direction, right);

  const center = box.getCenter(new Vector3());
  let minX = Infinity, maxX = -Infinity, minY = Infinity, maxY = -Infinity;
  for (let i = 0; i < 8; i++) {
    corner.set(
      i & 1 ? box.max.x : box.min.x,
      i & 2 ? box.max.y : box.min.y,
      i & 4 ? box.max.z : box.min.z
    ).sub(center);
    const x = corner.dot(right);
    const y = corner.dot(up);
    minX = Math.min(minX, x);
    maxX = Math.max(maxX, x);
    minY = Math.min(minY, y);
    maxY = Math.max(maxY, y);
  }

  // shift the target so the projected box is centered
  center.addScaledVector(right, (minX + maxX) / 2).addScaledVector(up, (minY + maxY) / 2);
  const halfWidth = (maxX - minX) / 2;
  const halfHeight = (maxY - minY) / 2;

  let distance = 0;
  for (let i = 0; i < 8; i++) {
    corner.set(
      i & 1 ? box.max.x : box.min.x,
      i & 2 ? box.max.y : box.min.y,
      i & 4 ? box.max.z : box.min.z
    ).sub(center);
    const depth = corner.dot(direction);
    const x = Math.abs(corner.dot(right));
    const y = Math.abs(corner.dot(up));
    distance = Math.max(distance, depth + Math.max(x, y) / tan);
  }
  distance *= PADDING;

  const radius = Math.max(halfWidth, halfHeight, box.getSize(corner).length() / 2);
  camera.position.copy(center).addScaledVector(direction, distance);
  camera.near = Math.max(distance - radius * 2, distance / 100);
  camera.far = distance + radius * 2;
  camera.lookAt(center);
  camera.updateProjectionMatrix();
}

export async function loadMatcap(url: string) {
  const response = await fetch(url);
  // TextureLoader flips images on upload, ImageBitmaps have to be flipped up front
  const bitmap = await createImageBitmap(await response.blob(), { imageOrientation: 'flipY' });
  const texture = new Texture(bitmap);
  texture.colorSpace = SRGBColorSpace;
  texture.flipY = false;
  texture.needsUpdate = true;
  return texture;
}

/**
 * Renders the encoded result of a graph execution into an image.
 * Works without a DOM, so it can be used inside of a worker.
 */
export class ThumbnailRenderer {
  private canvas = new OffscreenCanvas(1, 1);
  private renderer: WebGLRenderer;
  private material: MeshMatcapMaterial;
  private camera = new PerspectiveCamera(FOV, 1);

  constructor(matcap?: Texture) {
    this.renderer = new WebGLRenderer({
      canvas: this.canvas,
      antialias: true,
      alpha: true,
      preserveDrawingBuffer: true
    });
    this.renderer.setClearColor(0x000000, 0);
    // thumbnails show the colors of a mesh like the plant material of the viewer
    this.material = takeVertexColors(
      new MeshMatcapMaterial({ color: 0xffffff, matcap, side: DoubleSide, toneMapped: false })
    );
  }

  /** bounding rectangle of all non transparent pixels, y pointing down */
  private getContentBounds(size: number) {
    const gl = this.renderer.getContext();
    const pixels = new Uint8Array(size * size * 4);
    gl.readPixels(0, 0, size, size, gl.RGBA, gl.UNSIGNED_BYTE, pixels);

    let minX = size, minY = size, maxX = -1, maxY = -1;
    for (let y = 0; y < size; y++) {
      for (let x = 0; x < size; x++) {
        if (pixels[(y * size + x) * 4 + 3] === 0) continue;
        minX = Math.min(minX, x);
        maxX = Math.max(maxX, x);
        minY = Math.min(minY, y);
        maxY = Math.max(maxY, y);
      }
    }
    if (maxX < 0) return;

    // readPixels starts at the bottom left
    return { x: minX, y: size - 1 - maxY, width: maxX - minX + 1, height: maxY - minY + 1 };
  }

  /** Draws the box from `view` on the canvas, cropped to what was drawn. False when nothing was. */
  private drawView(scene: Scene, box: Box3, view: View, size: number) {
    const { direction, up } = VIEWS[view];
    this.camera.up.copy(up);
    this.camera.clearViewOffset();
    fitCameraToBox(this.camera, box, direction);

    this.renderer.setSize(size, size, false);
    this.renderer.render(scene, this.camera);

    // The box is only a rough fit, crop the view to the pixels that were actually drawn
    const bounds = this.getContentBounds(size);
    if (!bounds) return false;
    const crop = Math.max(bounds.width, bounds.height) * PADDING;
    this.camera.setViewOffset(
      size,
      size,
      bounds.x + bounds.width / 2 - crop / 2,
      bounds.y + bounds.height / 2 - crop / 2,
      crop,
      crop
    );
    this.renderer.render(scene, this.camera);
    return true;
  }

  /** The result in a scene, handed to `draw`, and freed again afterwards */
  private async withResult<T>(
    result: Int32Array,
    draw: (scene: Scene, box: Box3) => Promise<T | undefined>
  ): Promise<T | undefined> {
    const scene = new Scene();
    const group = new Group();
    scene.add(group);

    const inputs = splitNestedArray(result);
    createResultPools(group, this.material).update(inputs);

    try {
      const box = new Box3().setFromObject(group);
      if (box.isEmpty()) return;
      return await draw(scene, box);
    } finally {
      group.traverse((child) => {
        if ('isInstancedMesh' in child) (child as InstancedMesh).dispose();
        if ('geometry' in child) (child as Mesh).geometry.dispose();
      });
      this.renderer.renderLists.dispose();
    }
  }

  render(result: Int32Array, size: number): Promise<Blob | undefined> {
    return this.withResult(result, async (scene, box) => {
      if (!this.drawView(scene, box, 'default', size)) return;
      return this.canvas.convertToBlob({ type: 'image/webp', quality: 0.85 });
    });
  }

  /** The views next to each other in one PNG, each `size` pixels square and fitted on its own */
  renderViews(result: Int32Array, size: number, views: View[]): Promise<Blob | undefined> {
    return this.withResult(result, async (scene, box) => {
      const sheet = new OffscreenCanvas(size * views.length, size);
      const context = sheet.getContext('2d')!;
      views.forEach((view, i) => {
        if (this.drawView(scene, box, view, size)) context.drawImage(this.canvas, i * size, 0);
      });
      return sheet.convertToBlob({ type: 'image/png' });
    });
  }
}
