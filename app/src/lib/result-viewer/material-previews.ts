import { AgXToneMapping, Box3, Mesh, PerspectiveCamera, Scene, SphereGeometry, WebGLRenderer } from 'three';
import { matcapLoaded, materials, setHeightRange } from './materials';

const SIZE = 128;

let previews: Promise<Record<string, string>> | undefined;

/** renders a sphere for every material once, the images are reused afterwards */
export function getMaterialPreviews() {
  previews ??= renderPreviews();
  return previews;
}

async function renderPreviews() {
  await matcapLoaded;

  const canvas = document.createElement('canvas');
  const renderer = new WebGLRenderer({ canvas, antialias: true, alpha: true });
  // same as the default of the threlte canvas
  renderer.toneMapping = AgXToneMapping;
  renderer.setClearColor(0x000000, 0);
  renderer.setSize(SIZE, SIZE, false);

  const scene = new Scene();
  const geometry = new SphereGeometry(1, 32, 20);
  geometry.computeBoundingBox();
  const mesh = new Mesh(geometry);
  scene.add(mesh);

  // looking at the sphere from the same angle as the default viewer camera
  const camera = new PerspectiveCamera(30, 1);
  camera.position.set(2.6, 2.6, 2.6);
  camera.lookAt(0, 0, 0);

  const result: Record<string, string> = {};
  try {
    for (const entry of materials) {
      const material = entry.create();
      setHeightRange(material, geometry.boundingBox ?? new Box3());
      mesh.material = material;
      renderer.render(scene, camera);
      result[entry.id] = canvas.toDataURL('image/png');
      material.dispose();
    }
  } finally {
    geometry.dispose();
    renderer.dispose();
    renderer.forceContextLoss();
  }
  return result;
}
