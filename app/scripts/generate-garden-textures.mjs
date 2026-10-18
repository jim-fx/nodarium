// Generates the textures of the /garden route through OpenRouter.
// Usage: node scripts/generate-garden-textures.mjs [name...]
// Reads OPENROUTER_API_KEY from the environment or ../.env.test
import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const envFile = join(root, '..', '.env.test');
if (!process.env.OPENROUTER_API_KEY && existsSync(envFile)) {
  process.loadEnvFile(envFile);
}

const API_KEY = process.env.OPENROUTER_API_KEY;
if (!API_KEY) throw new Error('OPENROUTER_API_KEY is not set');

const MODEL = 'google/gemini-3.1-flash-image';
const OUT_DIR = join(root, 'static', 'garden');
const SIZE = 1024;

const STYLE = 'Square, perfectly top-down orthographic view, flat diffuse overcast lighting, '
  + 'no cast shadows, no vignette, no objects, no text, uniform detail across the whole frame '
  + 'so it can be used as a seamless tiling texture for a 3D renderer.';

// everything else comes from poly haven, see download-garden-assets.mjs
const textures = {
  hedge:
    'A photographic texture of a dense, neatly clipped yew hedge seen straight on, countless tiny '
    + 'dark green needles with lighter fresh tips and deep shadowy gaps, rich and lush. '
    + STYLE,
  moss: 'A photographic texture of dense, short cushion moss seen from directly above, deep muted '
    + 'olive green with darker gaps, soft and velvety. '
    + STYLE
};

async function generate(name, prompt) {
  const res = await fetch('https://openrouter.ai/api/v1/chat/completions', {
    method: 'POST',
    headers: {
      'Authorization': `Bearer ${API_KEY}`,
      'Content-Type': 'application/json'
    },
    body: JSON.stringify({
      model: MODEL,
      modalities: ['image', 'text'],
      image_config: { aspect_ratio: '1:1' },
      messages: [{ role: 'user', content: prompt }]
    })
  });
  if (!res.ok) throw new Error(`${name}: ${res.status} ${await res.text()}`);

  const json = await res.json();
  const url = json.choices?.[0]?.message?.images?.[0]?.image_url?.url;
  if (!url) throw new Error(`${name}: no image in response ${JSON.stringify(json).slice(0, 500)}`);

  const raw = join(OUT_DIR, `${name}.raw.png`);
  writeFileSync(raw, Buffer.from(url.split(',')[1], 'base64'));
  execFileSync('magick', [
    raw,
    '-resize',
    `${SIZE}x${SIZE}^`,
    '-gravity',
    'center',
    '-extent',
    `${SIZE}x${SIZE}`,
    '-quality',
    '82',
    join(OUT_DIR, `${name}.jpg`)
  ]);
  execFileSync('rm', [raw]);
  console.log(`generated ${name}.jpg`);
}

mkdirSync(OUT_DIR, { recursive: true });
const names = process.argv.slice(2).length ? process.argv.slice(2) : Object.keys(textures);
// the model sometimes refuses with IMAGE_RECITATION, simply asking again usually works
async function generateWithRetry(name, attempts = 4) {
  for (let i = 1; i <= attempts; i++) {
    try {
      return await generate(name, textures[name]);
    } catch (error) {
      if (i === attempts) throw error;
      console.warn(`${name}: attempt ${i} failed, retrying`);
    }
  }
}

await Promise.all(names.map((name) => generateWithRetry(name)));
