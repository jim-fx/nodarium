// Downloads the CC0 textures and the HDRI of the /garden route from Poly Haven.
// Usage: node scripts/download-garden-assets.mjs
import { execFileSync } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const OUT_DIR = join(root, 'static', 'garden');
const RESOLUTION = '1k';

/** poly haven asset id and the maps that are used per material of the garden */
const textures = {
  // the raked pattern is a bump map, three.js can not combine it with a normal map
  gravel: { id: 'ground_grey', maps: ['Diffuse', 'arm'] },
  rock: { id: 'mossy_rock', maps: ['Diffuse', 'nor_gl', 'arm'] },
  planks: { id: 'dark_planks', maps: ['Diffuse', 'nor_gl', 'arm'] },
  forest: { id: 'forest_leaves_02', maps: ['Diffuse', 'nor_gl', 'arm'] },
  soil: { id: 'farm_soil', maps: ['Diffuse', 'nor_gl', 'arm'] },
  boards: { id: 'brown_planks_09', maps: ['Diffuse', 'nor_gl', 'arm'] },
  bricks: { id: 'brick_floor_02', maps: ['Diffuse', 'nor_gl', 'arm'] },
  lawn: { id: 'leafy_grass', maps: ['Diffuse', 'nor_gl', 'arm'] }
};
/** one hdri per style of the garden */
const hdris = { zen: 'forest_grove', cottage: 'green_sanctuary' };
/** models planted around the bed */
const models = ['fern_02', 'rock_moss_set_02'];

/** poly haven map name -> file suffix and jpeg quality */
const maps = {
  Diffuse: { suffix: 'diff', quality: 80 },
  nor_gl: { suffix: 'nor', quality: 90 },
  // ambient occlusion, roughness and metalness packed into r, g and b
  arm: { suffix: 'arm', quality: 85 }
};

async function download(url, target) {
  const res = await fetch(url);
  if (!res.ok) throw new Error(`${url}: ${res.status}`);
  writeFileSync(target, Buffer.from(await res.arrayBuffer()));
}

async function files(id) {
  const res = await fetch(`https://api.polyhaven.com/files/${id}`);
  if (!res.ok) throw new Error(`${id}: ${res.status}`);
  return res.json();
}

mkdirSync(OUT_DIR, { recursive: true });

await Promise.all(
  Object.entries(textures).map(async ([name, { id, maps: used }]) => {
    const available = await files(id);
    for (const map of used) {
      const { suffix, quality } = maps[map];
      const target = join(OUT_DIR, `${name}_${suffix}.jpg`);
      await download(available[map][RESOLUTION].jpg.url, target);
      execFileSync('magick', [target, '-strip', '-quality', String(quality), target]);
    }
    console.log(`downloaded ${name} (${id})`);
  })
);

await Promise.all(
  Object.entries(hdris).map(async ([style, id]) => {
    const available = await files(id);
    await download(available.hdri[RESOLUTION].hdr.url, join(OUT_DIR, `environment-${style}.hdr`));
    console.log(`downloaded environment ${style} (${id})`);
  })
);

await Promise.all(models.map(async (id) => {
  const available = await files(id);
  const { gltf } = available.gltf[RESOLUTION];
  const dir = join(OUT_DIR, 'models', id);
  mkdirSync(join(dir, 'textures'), { recursive: true });
  await download(gltf.url, join(dir, `${id}.gltf`));
  for (const [path, { url }] of Object.entries(gltf.include)) {
    await download(url, join(dir, path));
  }
  // the gltf export uses jpegs without alpha, the cut out mask is a separate map
  if (available.Alpha) {
    await download(
      available.Alpha[RESOLUTION].jpg.url,
      join(dir, 'textures', `${id}_alpha_${RESOLUTION}.jpg`)
    );
  }
  console.log(`downloaded model ${id}`);
}));
