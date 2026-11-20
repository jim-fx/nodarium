import { appSettings } from '#lib/settings/app-settings.svelte.ts';
import { Color, LinearSRGBColorSpace } from 'three';

const variables = [
  'layer-0',
  'layer-1',
  'layer-2',
  'layer-3',
  'outline',
  'text',
  'active',
  'selected',
  'connection',
  'error',
  'frame-gray',
  'frame-green',
  'frame-brown',
  'frame-blue',
  'frame-red',
  'frame-violet'
] as const;

function getColor(variable: (typeof variables)[number]) {
  const style = getComputedStyle(document.body.parentElement!);
  const color = style.getPropertyValue(`--color-${variable}`);
  return new Color().setStyle(color, LinearSRGBColorSpace);
}

/** The bar on the header of a node of these collections; core nodes belong everywhere and get none */
export const systemColors: Record<string, string> = {
  'max/plantarium': '#6f8a52',
  'max/myzel': '#9a7650'
};

export const colors = Object.fromEntries(
  variables.map((v) => [v, getColor(v)])
) as Record<(typeof variables)[number], Color>;

$effect.root(() => {
  $effect(() => {
    if (!appSettings.value.theme || !('getComputedStyle' in globalThis)) return;
    const style = getComputedStyle(document.body.parentElement!);
    for (const v of variables) {
      const hex = style.getPropertyValue(`--color-${v}`);
      colors[v].setStyle(hex, LinearSRGBColorSpace);
    }
  });
});
