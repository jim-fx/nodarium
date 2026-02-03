import { appSettings } from '$lib/settings/app-settings.svelte';
import { Color, LinearSRGBColorSpace } from 'three';

const variables = [
  'layer-0',
  'layer-1',
  'layer-2',
  'layer-3',
  'outline',
  'active',
  'selected',
  'edge'
] as const;

function getColor(variable: (typeof variables)[number]) {
  const style = getComputedStyle(document.body.parentElement!);
  const color = style.getPropertyValue(`--color-${variable}`);
  return new Color().setStyle(color, LinearSRGBColorSpace);
}

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
