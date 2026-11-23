import { Color, LinearSRGBColorSpace } from 'three';
import type { ThemeColors } from './options';

// a record, so a colour added to ThemeColors has to be added here too
const NAMES = Object.keys(
  {
    'layer-0': true,
    'layer-1': true,
    'layer-2': true,
    outline: true,
    active: true,
    selected: true,
    error: true,
    'frame-gray': true,
    'frame-green': true,
    'frame-brown': true,
    'frame-blue': true,
    'frame-red': true,
    'frame-violet': true
  } satisfies Record<keyof ThemeColors, true>
) as (keyof ThemeColors)[];

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
