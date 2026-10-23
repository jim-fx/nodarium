import {
  createGeometryPool,
  createInstancedGeometryPool
} from '#lib/result-viewer/geometry/geometryPool.ts';
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
// Same viewing angle as the default camera of the viewer
const CAMERA_DIRECTION = new Vector3(1, 1, 1).normalize();
const PADDING = 1.08;

const right = new Vector3();
const up = new Vector3();
const corner = new Vector3();

/**
 * Moves the camera along CAMERA_DIRECTION until all corners of the box are in view,
 * and centers the view on the projected box (tighter than fitting a bounding sphere)
 */
function fitCameraToBox(camera: PerspectiveCamera, box: Box3) {
  const tan = Math.tan((FOV / 2) * (Math.PI / 180));
  right.crossVectors(camera.up, CAMERA_DIRECTION).normalize();
  up.crossVectors(CAMERA_DIRECTION, right);

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
    const depth = corner.dot(CAMERA_DIRECTION);
    const x = Math.abs(corner.dot(right));
    const y = Math.abs(corner.dot(up));
    distance = Math.max(distance, depth + Math.max(x, y) / tan);
  }
  distance *= PADDING;

  const radius = Math.max(halfWidth, halfHeight, box.getSize(corner).length() / 2);
  camera.position.copy(center).addScaledVector(CAMERA_DIRECTION, distance);
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
    this.material = new MeshMatcapMaterial({ color: 0xffffff, matcap, side: DoubleSide });
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

  async render(result: Int32Array, size: number): Promise<Blob | undefined> {
    const scene = new Scene();
    const group = new Group();
    scene.add(group);

    const inputs = splitNestedArray(result);
    createGeometryPool(group, this.material).update(inputs.filter((i) => i[0] === 1));
    createInstancedGeometryPool(group, this.material).update(inputs.filter((i) => i[0] === 2));

    try {
      const box = new Box3().setFromObject(group);
      if (box.isEmpty()) return;

      fitCameraToBox(this.camera, box);

      this.renderer.setSize(size, size, false);
      this.camera.clearViewOffset();
      this.renderer.render(scene, this.camera);

      // The box is only a rough fit, crop the view to the pixels that were actually drawn
      const bounds = this.getContentBounds(size);
      if (!bounds) return;
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

      return await this.canvas.convertToBlob({ type: 'image/webp', quality: 0.85 });
    } finally {
      group.traverse((child) => {
        if ('isInstancedMesh' in child) (child as InstancedMesh).dispose();
        if ('geometry' in child) (child as Mesh).geometry.dispose();
      });
      this.renderer.renderLists.dispose();
    }
  }
}
