import { createGeometryPool, createInstancedGeometryPool } from '#lib/result-viewer/geometryPool.ts';
import { splitNestedArray } from '@nodarium/utils';
import { Box3, Group, type InstancedMesh, type Material, type Mesh } from 'three';
import { PLANT_MAX_HEIGHT, PLANT_MAX_RADIUS } from './config';

/**
 * Builds the meshes of an executed project.
 * The plant is scaled to fit into the size budget of the garden. The origin and ground
 * of the editor stay the root of the plant, branches that hang lower disappear in the sand.
 */
export function createPlantMesh(result: Int32Array, material: Material) {
  const content = new Group();
  const inputs = splitNestedArray(result);
  createGeometryPool(content, material).update(inputs.filter((i) => i[0] === 1));
  createInstancedGeometryPool(content, material).update(inputs.filter((i) => i[0] === 2));

  const box = new Box3().setFromObject(content);
  if (box.isEmpty()) return;

  // the plant grows from the origin of the editor, so it stays the center of the plant
  const radius = Math.max(-box.min.x, box.max.x, -box.min.z, box.max.z);
  // only when nothing is above the ground the plant is lifted onto it
  const ground = box.max.y > 0 ? 0 : box.min.y;
  const height = box.max.y - ground;
  const scale = Math.min(
    PLANT_MAX_HEIGHT / Math.max(height, 1e-6),
    PLANT_MAX_RADIUS / Math.max(radius, 1e-6)
  );

  content.position.y = -ground;
  content.traverse((child) => {
    child.castShadow = true;
    child.receiveShadow = true;
  });

  const plant = new Group();
  plant.add(content);
  plant.scale.setScalar(scale);

  return { plant, height: height * scale };
}

export function disposePlantMesh(plant: Group) {
  plant.traverse((child) => {
    if ('isInstancedMesh' in child) (child as InstancedMesh).dispose();
    if ('geometry' in child) (child as Mesh).geometry.dispose();
  });
}
