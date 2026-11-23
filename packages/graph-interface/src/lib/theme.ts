import { Color, LinearSRGBColorSpace } from 'three';
import type { ThemeColors } from './options';

const NAMES = [
  'layer-0',
  'layer-1',
  'layer-2',
  'outline',
  'active',
  'selected',
  'error',
  'frame-gray',
  'frame-green',
  'frame-brown',
  'frame-blue',
  'frame-red',
  'frame-violet'
] as const satisfies (keyof ThemeColors)[];

/**
 * The colours of the editor from the `--color-*` variables of the theme of `@nodarium/ui/app.css`.
 * Pass the colours of the editor again after the theme changed, they are updated in place.
 */
export function readThemeColors(
  colors?: ThemeColors,
  element: Element = document.documentElement
): ThemeColors {
  const style = getComputedStyle(element);
  const result = colors ?? ({} as ThemeColors);
  for (const name of NAMES) {
    const value = style.getPropertyValue(`--color-${name}`).trim() || '#000';
    (result[name] ??= new Color()).setStyle(value, LinearSRGBColorSpace);
  }
  return result;
}
