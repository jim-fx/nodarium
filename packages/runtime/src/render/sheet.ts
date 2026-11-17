import { BACKGROUND, type Image } from './rasterize';

const GAP_COLOR = [205, 200, 188];
const LABEL_COLOR = [70, 62, 52];

/**
 * 3 × 5 pixels per character, a row of three bits per string: enough for "s12 t0.25". Other
 * characters are left blank.
 */
const GLYPHS: Record<string, string[]> = {
  '0': ['111', '101', '101', '101', '111'],
  '1': ['010', '110', '010', '010', '111'],
  '2': ['111', '001', '111', '100', '111'],
  '3': ['111', '001', '111', '001', '111'],
  '4': ['101', '101', '111', '001', '001'],
  '5': ['111', '100', '111', '001', '111'],
  '6': ['111', '100', '111', '101', '111'],
  '7': ['111', '001', '010', '010', '010'],
  '8': ['111', '101', '111', '101', '111'],
  '9': ['111', '101', '111', '001', '111'],
  '.': ['000', '000', '000', '000', '010'],
  '-': ['000', '000', '111', '000', '000'],
  s: ['000', '111', '100', '001', '111'],
  t: ['010', '111', '010', '010', '011'],
  ' ': ['000', '000', '000', '000', '000']
};

/** Writes `text` into an image at `x`, `y`, every pixel of a glyph `scale` pixels square */
function drawText(image: Image, text: string, x: number, y: number, scale: number) {
  for (const char of text) {
    (GLYPHS[char] ?? GLYPHS[' ']).forEach((row, gy) => {
      [...row].forEach((bit, gx) => {
        if (bit !== '1') return;
        for (let dy = 0; dy < scale; dy++) {
          for (let dx = 0; dx < scale; dx++) {
            const px = x + gx * scale + dx, py = y + gy * scale + dy;
            if (px < image.width && py < image.height) {
              image.pixels.set(LABEL_COLOR, (py * image.width + px) * 3);
            }
          }
        }
      });
    });
    x += 4 * scale;
  }
}

export type SheetLayout = {
  columns: number;
  rows: number;
  /** the size of one cell, without the gap between cells */
  cellWidth: number;
  cellHeight: number;
  gap: number;
};

/** Where the cell of `index` starts in the sheet */
export function cellOrigin(layout: SheetLayout, index: number) {
  const column = index % layout.columns, row = Math.floor(index / layout.columns);
  return {
    column,
    row,
    x: column * (layout.cellWidth + layout.gap),
    y: row * (layout.cellHeight + layout.gap)
  };
}

/**
 * The cells in rows of `layout.columns`, each with its label in the top left corner. A cell
 * without an image stays empty.
 */
export function composeSheet(
  cells: { image?: Image; label?: string }[],
  layout: SheetLayout
): Image {
  const { columns, rows, cellWidth, cellHeight, gap } = layout;
  const width = columns * cellWidth + (columns - 1) * gap;
  const height = rows * cellHeight + (rows - 1) * gap;
  const sheet: Image = { pixels: new Uint8Array(width * height * 3), width, height };
  for (let i = 0; i < sheet.pixels.length; i += 3) sheet.pixels.set(GAP_COLOR, i);
  const scale = Math.max(2, Math.round(cellHeight / 128));
  cells.forEach(({ image, label }, index) => {
    const { x, y } = cellOrigin(layout, index);
    for (let row = 0; row < cellHeight; row++) {
      const at = ((y + row) * width + x) * 3;
      if (image) {
        sheet.pixels.set(
          image.pixels.subarray(row * image.width * 3, (row + 1) * image.width * 3),
          at
        );
      } else {
        for (let i = 0; i < cellWidth; i++) sheet.pixels.set(BACKGROUND, at + i * 3);
      }
    }
    if (label) drawText(sheet, label, x + 3 * scale, y + 3 * scale, scale);
  });
  return sheet;
}
