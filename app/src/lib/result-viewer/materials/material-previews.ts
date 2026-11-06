import {
  AgXToneMapping,
  Box3,
  Float32BufferAttribute,
  Mesh,
  PerspectiveCamera,
  Scene,
  SphereGeometry,
  WebGLRenderer
} from 'three';
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
  // bands of rings for the groups material, the other materials ignore it
  const rings = geometry.parameters.heightSegments + 1;
  const perRing = geometry.parameters.widthSegments + 1;
  const groups = new Float32Array(rings * perRing);
  for (let i = 0; i < groups.length; i++) groups[i] = Math.floor(Math.floor(i / perRing) / 4);
  geometry.setAttribute('group', new Float32BufferAttribute(groups, 1));
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
