import type { Graph } from '@nodarium/types';
import { fileURLToPath } from 'node:url';
import { inflateSync } from 'node:zlib';
import { describe, expect, it } from 'vitest';
import { CURRENT_GRAPH_VERSION } from '../graph-migrations';
import { FileNodeRegistry } from '../registry/file-node-registry';
import { MemoryRuntimeExecutor } from '../runtime-executor';
import { renderSheetToPng, renderToPng } from '.';

const nodesDir = fileURLToPath(new URL('../../../../app/static/nodes', import.meta.url));

async function stem() {
  const runtime = new MemoryRuntimeExecutor(new FileNodeRegistry(nodesDir));
  const graph: Graph = {
    id: 0,
    version: CURRENT_GRAPH_VERSION,
    nodes: [
      { id: 1, type: 'max/plantarium/stem', props: { length: 0.5, thickness: 0.05 } },
      { id: 2, type: 'max/plantarium/output' }
    ],
    edges: [[1, 0, 2, 'input']],
    groups: []
  };
  return (await runtime.run(graph, { seed: 1 })).result;
}

/** The RGB pixel at x, y of a PNG that `encodePng` wrote */
function pixel(png: Uint8Array, x: number, y: number) {
  const buffer = Buffer.from(png);
  const width = buffer.readUInt32BE(16);
  const rows = inflateSync(buffer.subarray(41, buffer.length - 16));
  const at = y * (width * 3 + 1) + 1 + x * 3;
  return [...rows.subarray(at, at + 3)];
}

describe('renderToPng', () => {
  it('draws the views of the default when they are named', async () => {
    const result = await stem();
    const plain = renderToPng(result, { size: 48 })!;
    expect(renderToPng(result, { size: 48, views: ['front', 'side', 'top'] })!.png).toEqual(
      plain.png
    );
    expect(renderToPng(result, { size: 48, views: ['front', 'side', 'top', 'below'] })!.png)
      .toEqual(renderToPng(result, { size: 48, below: true })!.png);
    expect(() => renderToPng(result, { views: [] })).toThrow('the views are front, side');
  });
});

describe('renderSheetToPng', () => {
  it('leaves a cell without a result empty and labels every cell', async () => {
    const result = await stem();
    const sheet = renderSheetToPng(
      [{ result, label: 's1' }, { label: 's2' }],
      { size: 64, views: ['front'] }
    )!;
    expect(sheet).toMatchObject({ width: 130, height: 64, columns: 2, rows: 1 });
    expect(sheet.cells[1]).toEqual({ column: 1, row: 0, x: 66, y: 0, stats: null });
    expect(sheet.cells[0].stats!.height).toBeCloseTo(0.5, 2);
    // the top bar of the "s" of the label, two pixels per pixel of the font
    expect(pixel(sheet.png, 66 + 6, 6 + 2)).toEqual([70, 62, 52]);
    expect(pixel(sheet.png, 66 + 40, 40)).toEqual([236, 233, 224]);
    expect(pixel(sheet.png, 64, 10)).toEqual([205, 200, 188]);

    expect(renderSheetToPng([{ label: 's1' }])).toBeNull();
  });
});
