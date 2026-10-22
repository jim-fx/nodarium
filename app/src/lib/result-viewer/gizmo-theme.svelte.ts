import { colors } from '#lib/graph-interface/graph/colors.svelte.ts';
import { appSettings } from '#lib/settings/app-settings.svelte.ts';
import { Color, SRGBColorSpace } from 'three';

/** `colors` holds the CSS values as they are, the materials of three.js want its working color space */
function forMaterial(token: Color) {
  return new Color().setRGB(token.r, token.g, token.b, SRGBColorSpace);
}

function luminance(token: Color) {
  return 0.2126 * token.r + 0.7152 * token.g + 0.0722 * token.b;
}

/** The colors of the gizmos, they follow the theme of the app */
class GizmoTheme {
  /** the marks of the node: spheres, dots and lines */
  accent = $state.raw(new Color());
  /** the handles of the transform controls */
  handle = $state.raw(new Color());
  /** the handle under the mouse */
  hover = $state.raw(new Color());
  /** light themes need a stronger fill, the same opacity is much harder to see on white */
  opacityScale = $state(1);

  constructor() {
    this.update();
  }

  update() {
    this.accent = forMaterial(colors['selected']);
    this.handle = forMaterial(colors['layer-3']);
    this.hover = forMaterial(colors['active']);
    this.opacityScale = luminance(colors['layer-0']) > 0.5 ? 2 : 1;
  }

  /** an opacity that reads the same on every theme */
  opacity(base: number) {
    return Math.min(base * this.opacityScale, 1);
  }
}

export const gizmoTheme = new GizmoTheme();

// The colors module updates its colors when the theme changes. This effect is created after
// that one, so it always sees the new values.
$effect.root(() => {
  $effect(() => {
    void appSettings.value.theme;
    gizmoTheme.update();
  });
});
