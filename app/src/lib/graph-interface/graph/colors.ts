type Color = { hue: number; saturation: number; lightness: number };

// The edges read these values as linear and the canvas shows them as sRGB, so
// hsl(80, 80%, 20%) is drawn as #8aa238. The hex is what the edge looks like.
export const LINK_COLORS: Record<string, Color> = {
  float: { hue: 70, saturation: 0, lightness: 10 }, // #595959
  integer: { hue: 0, saturation: 0, lightness: 19 }, // #7a7a7a
  vec3: { hue: 165, saturation: 61, lightness: 50 }, // #7ce8d4
  color: { hue: 120, saturation: 100, lightness: 60 }, // #7cff7c
  shape: { hue: 30, saturation: 100, lightness: 60 }, // #ffcb7c
  path: { hue: 80, saturation: 80, lightness: 20 }, // #8aa238
  points: { hue: 214, saturation: 100, lightness: 60 }, // #7cc4ff
  force: { hue: 237, saturation: 100, lightness: 63 }, // #8c96ff
  geometry: { hue: 0, saturation: 70, lightness: 50 }, // #ed6c6c
  mesh: { hue: 335, saturation: 100, lightness: 63 }, // #ff8cc6
  pattern: { hue: 264, saturation: 100, lightness: 63 }, // #c58cff
  '*': { hue: 200, saturation: 100, lightness: 20 } // #2e7fb8
};

export class ColorGenerator {
  private colors: Map<string, Color> = new Map();

  constructor(predefined: Record<string, Color> = LINK_COLORS) {
    for (const [id, color] of Object.entries(predefined)) {
      this.colors.set(id, color);
    }
  }

  public getColors() {
    return Object.fromEntries(
      this.colors.entries().map(([key, col]) => {
        return [key, this.colorToHsl(col)];
      })
    );
  }

  public getColor(id: string): string {
    let color = this.colors.get(id);
    if (!color) {
      color = this.generateColor(id);
      this.colors.set(id, color);
    }
    return this.colorToHsl(color);
  }

  // from the name, so an unknown type keeps its colour whatever is drawn first
  private generateColor(id: string): Color {
    let hash = 0;
    for (let i = 0; i < id.length; i++) {
      hash = (hash * 31 + id.charCodeAt(i)) | 0;
    }
    return { hue: Math.abs(hash) % 360, saturation: 100, lightness: 60 };
  }

  private colorToHsl(c: Color): string {
    return `hsl(${c.hue}, ${c.saturation}%, ${c.lightness}%)`;
  }
}
