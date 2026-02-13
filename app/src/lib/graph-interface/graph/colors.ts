type Color = { hue: number; saturation: number; lightness: number };

export class ColorGenerator {
  private colors: Map<string, Color> = new Map();
  private lightnessLevels = [10, 60];

  constructor(predefined: Record<string, Color>) {
    for (const [id, colorStr] of Object.entries(predefined)) {
      this.colors.set(id, colorStr);
    }
  }

  public getColor(id: string): string {
    if (this.colors.has(id)) {
      return this.colorToHsl(this.colors.get(id)!);
    }

    const newColor = this.generateNewColor();
    this.colors.set(id, newColor);
    return this.colorToHsl(newColor);
  }

  private generateNewColor(): Color {
    const existingHues = Array.from(this.colors.values()).map(c => c.hue).sort();
    let hue = existingHues[0];
    let attempts = 0;

    while (
      existingHues.some(h => Math.abs(h - hue) < 30 || Math.abs(h - hue) > 330)
      && attempts < 360
    ) {
      hue = (hue + 30) % 360;
      attempts++;
    }

    const lightness = 60;

    return { hue, lightness, saturation: 100 };
  }

  private colorToHsl(c: Color): string {
    return `hsl(${c.hue}, ${c.saturation}%, ${c.lightness}%)`;
  }
}
