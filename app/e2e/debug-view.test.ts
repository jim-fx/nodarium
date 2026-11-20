import { expect, type Page, test } from '@playwright/test';

// a rosette of leaves, the leaf itself is watched by a debug node as well
const rosette = {
  version: 14,
  nodes: [
    { id: 1, type: 'max/plantarium/spiral', props: { amount: 5 } },
    { id: 2, type: 'max/plantarium/leaf', props: { size: 0.01 } },
    { id: 3, type: 'max/plantarium/instance_from_points', props: { angle: 1.2 } },
    { id: 4, type: 'max/plantarium/output' }
  ],
  edges: [[1, 0, 3, 'points'], [2, 0, 3, 'geometry'], [3, 0, 4, 'input']]
};

const DEBUG_NODE = '__internal/node/debug';
// the wireframe the debug node drew geometry with before
const WIREFRAME = [0x4d, 0xd2, 0xff];

/** How many pixels of the viewer are bright and coloured, and how many in the old wireframe */
async function viewerPixels(page: Page) {
  const png = await page.locator('[data-planty="viewer"] canvas').screenshot();
  return page.evaluate(async ([base64, wire]) => {
    const blob = await (await fetch(`data:image/png;base64,${base64}`)).blob();
    const bitmap = await createImageBitmap(blob);
    const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
    const context = canvas.getContext('2d')!;
    context.drawImage(bitmap, 0, 0);
    const { data } = context.getImageData(0, 0, bitmap.width, bitmap.height);
    let colored = 0;
    let wireframe = 0;
    for (let i = 0; i < data.length; i += 4) {
      const [r, g, b] = [data[i], data[i + 1], data[i + 2]];
      if (Math.max(r, g, b) > 80 && Math.max(r, g, b) - Math.min(r, g, b) > 30) colored++;
      if (Math.abs(r - wire[0]) + Math.abs(g - wire[1]) + Math.abs(b - wire[2]) < 40) wireframe++;
    }
    return { colored, wireframe };
  }, [png.toString('base64'), WIREFRAME] as const);
}

test('a debug node on a geometry shows it shaded, and alone in the local view', async ({ page }) => {
  await page.setViewportSize({ width: 1600, height: 900 });
  await page.goto('http://localhost:4173/?tutorial=0');
  await page.waitForFunction(() => window.nodarium, null, { polling: 100 });
  await page.evaluate(() => window.nodarium!.ready);
  await page.evaluate((g) => window.nodarium!.graph.load(g, { title: 'debug' }), rosette);
  await page.evaluate(() => window.nodarium!.idle());

  const viewer = page.locator('[data-debug-view]');
  await expect(viewer).toHaveCount(0);

  const debugId = await page.evaluate((type) => {
    const id = window.nodarium!.graph.addNode(type, {});
    window.nodarium!.graph.connect(2, 0, id, 'input');
    return id;
  }, DEBUG_NODE);
  await page.evaluate(() => window.nodarium!.idle());

  // the leaf is drawn in front of the dimmed rosette, with the material of the viewer
  await expect(viewer).toHaveAttribute('data-debug-view', 'with-result');
  await expect(async () => {
    const pixels = await viewerPixels(page);
    expect(pixels.colored).toBeGreaterThan(200);
    expect(pixels.wireframe).toBe(0);
  }).toPass({ timeout: 8000 });

  // I on the selected debug node shows only the leaf, framed
  const debugNode = page.locator(`.node[data-node-id="${debugId}"]`);
  await page.locator('.graph-wrapper').focus();
  await page.keyboard.press('.');
  // the camera glides to the nodes, the node is clicked once it stays where it is
  let last = '';
  await expect.poll(async () => {
    const at = JSON.stringify(await debugNode.boundingBox());
    const settled = at === last;
    last = at;
    return settled;
  }, { intervals: [100] }).toBe(true);
  const box = (await debugNode.boundingBox())!;
  await page.mouse.click(box.x + 10, box.y + 5);
  await expect(debugNode).toHaveClass(/active/);
  await page.mouse.move(1, 1);
  await page.keyboard.press('i');
  await expect(viewer).toHaveAttribute('data-debug-view', 'only');
  await expect(async () => {
    const pixels = await viewerPixels(page);
    expect(pixels.colored).toBeGreaterThan(200);
    expect(pixels.wireframe).toBe(0);
  }).toPass({ timeout: 8000 });

  await page.keyboard.press('i');
  await expect(viewer).toHaveAttribute('data-debug-view', 'with-result');

  // without the debug node the rosette is shown as it is
  await page.evaluate((id) => window.nodarium!.graph.removeNode(id), debugId);
  await page.evaluate(() => window.nodarium!.idle());
  await expect(viewer).toHaveCount(0);
});
