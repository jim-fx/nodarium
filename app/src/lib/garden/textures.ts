import {
  EquirectangularReflectionMapping,
  MirroredRepeatWrapping,
  RepeatWrapping,
  SRGBColorSpace,
  type Texture,
  TextureLoader
} from 'three';
import { HDRLoader } from 'three/examples/jsm/loaders/HDRLoader.js';
import type { GardenStyle } from './config';

const loader = new TextureLoader();

function load(url: string, repeat: number) {
  const texture = loader.load(url);
  texture.wrapS = texture.wrapT = RepeatWrapping;
  texture.repeat.set(repeat, repeat);
  texture.anisotropy = 8;
  return texture;
}

/** These are generated, they are not seamless, so mirrored wrapping hides the seams */
export function loadGeneratedTexture(name: 'moss' | 'hedge', repeat = 1): Texture {
  const texture = load(`/garden/${name}.jpg`, repeat);
  texture.colorSpace = SRGBColorSpace;
  texture.wrapS = texture.wrapT = MirroredRepeatWrapping;
  return texture;
}

/**
 * Loads the maps of one of the poly haven materials, ready to be spread into a
 * MeshStandardMaterial. The arm texture packs ambient occlusion, roughness and metalness,
 * none of the materials are metallic, so only the first two are used.
 */
export function loadSurface(
  name: 'gravel' | 'rock' | 'planks' | 'forest' | 'soil' | 'boards' | 'bricks' | 'lawn',
  repeat = 1
) {
  const map = load(`/garden/${name}_diff.jpg`, repeat);
  map.colorSpace = SRGBColorSpace;
  const arm = load(`/garden/${name}_arm.jpg`, repeat);
  // the gravel is bump mapped with the rake pattern instead
  const normalMap = name === 'gravel' ? null : load(`/garden/${name}_nor.jpg`, repeat);
  return { map, normalMap, aoMap: arm, roughnessMap: arm };
}

export type Surface = ReturnType<typeof loadSurface>;

export async function loadEnvironment(style: GardenStyle) {
  const texture = await new HDRLoader().loadAsync(`/garden/environment-${style}.hdr`);
  texture.mapping = EquirectangularReflectionMapping;
  return texture;
}
