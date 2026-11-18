import { expect, type Page, test } from '@playwright/test';
import type { NodariumApi } from '../src/lib/agent-api/types.ts';

// nodes outside of the camera are hidden, the new project needs the room
test.use({ viewport: { width: 1920, height: 1080 } });

async function open(page: Page, search = '?tutorial=0') {
  await page.goto(`http://localhost:4173/${search}`);
  await page.waitForFunction(() => window.nodarium, null, { polling: 100 });
  await page.evaluate(() => window.nodarium!.ready);
}

test('a project opens from the menu, builds and shows what it built', async ({ page }) => {
  page.on('console', msg => {
    console.log(`[Browser Console] ${msg.type()}: ${msg.text()}`);
  });
  await open(page);

  await page.getByRole('button', { name: 'File', exact: true }).click();
  await page.getByRole('menuitem', { name: /^Open…/ }).click();
  const projectBrowser = page.getByRole('dialog', { name: 'Projects' });
  await projectBrowser.getByRole('button', { name: 'Default Plant', exact: true }).click();
  await expect(projectBrowser).toBeHidden();

  const graph = await page.evaluate(async () => {
    await window.nodarium!.idle();
    return window.nodarium!.graph.get();
  });
  expect(graph.meta?.title).toBe('Default Plant');
  const props = (id: number) => graph.nodes.find((n) => n.id === id)?.props;
  expect(props(10)).toMatchObject({ amount: 6, length: 0.55, thickness: 0.005 });
  expect(props(15)).toMatchObject({ scale: 44, strength: 0.245 });

  // the default plant is larger than the view, its nodes only have to be in the page
  for (const id of [10, 15, 9]) {
    await expect(page.locator(`div.wrapper[data-node-id="${id}"]`)).toBeAttached();
  }
  const length = page.locator(
    'div.wrapper[data-node-type="max/plantarium/stem"][data-node-input="length"] input'
  );
  expect(parseFloat(await length.inputValue())).toBe(0.55);

  const result = await page.evaluate(() => window.nodarium!.lastResult()!);
  expect(result.ok).toBe(true);
  expect(result.faces).toBeGreaterThan(0);
  const status = page.getByRole('status').filter({ hasText: 'vertices' });
  await expect(status).toContainText(`faces`);
  await expect(status).toContainText(
    result.vertices < 1000 ? `${result.vertices} vertices` : 'K vertices'
  );

  await page.getByRole('button', { name: 'File', exact: true }).click();
  await page.getByRole('menuitem', { name: /^New project/ }).click();
  const types = await page.evaluate(async () => {
    await window.nodarium!.idle();
    return window.nodarium!.graph.get().nodes.map((n) => n.type).sort();
  });
  expect(types).toEqual(['max/plantarium/output', 'max/plantarium/stem']);
  const nodes = page.locator('div.wrapper[data-node-id]');
  await expect(nodes).toHaveCount(2);
  await expect(nodes.and(page.locator('[data-node-type="max/plantarium/stem"]'))).toBeVisible();
  await expect(nodes.and(page.locator('[data-node-type="max/plantarium/output"]'))).toBeVisible();
});

test('the session of an agent: load a graph it wrote and read what it built', async ({ page }) => {
  await open(page);
  const result = await page.evaluate(async () => {
    const nodarium: NodariumApi = window.nodarium!;
    await nodarium.ready;
    return nodarium.graph.load({
      version: 13,
      nodes: [
        { id: 1, type: 'max/plantarium/stem', props: { amount: 4, length: 0.6 } },
        { id: 2, type: 'max/plantarium/noise', props: { strength: 0.4 } },
        { id: 3, type: 'max/plantarium/output' }
      ],
      edges: [[1, 0, 2, 'plant'], [2, 0, 3, 'input']]
    }, { title: 'agent test' });
  });
  expect(result).toMatchObject({ ok: true, issues: [] });
  expect(result.vertices).toBeGreaterThan(0);
  expect(result.faces).toBeGreaterThan(0);
  expect(result.bounds?.max[1]).toBeGreaterThan(0.3);
  expect(Object.keys(result.ms.nodes).sort()).toEqual(['1', '2', '3']);
  await expect(page.getByRole('button', { name: 'agent test' })).toBeVisible();
});

test('?tutorial=0 keeps Planty closed and remembers nothing', async ({ page }) => {
  await open(page);
  await expect(page.getByRole('dialog', { name: 'Planty' })).toBeHidden();
  expect(await page.evaluate(() => localStorage.getItem('planty:nodarium-tutorial:seen'))).toBe(
    null
  );

  await open(page, '');
  await expect(page.getByRole('dialog', { name: 'Planty' })).toBeVisible();
});

test('a page without animation frames still shows the result', async ({ page }) => {
  // a window that is not visible gets no frames, as in a browser an agent drives
  await page.addInitScript(() => {
    window.requestAnimationFrame = () => 0;
    window.cancelAnimationFrame = () => {};
  });
  await open(page);
  await page.evaluate(() => window.nodarium!.graph.loadTemplate('fern'));

  const viewer = page.locator('[data-planty="viewer"]');
  const lit = async () => {
    const shot = (await viewer.screenshot()).toString('base64');
    return page.evaluate(async (png) => {
      const blob = await (await fetch(`data:image/png;base64,${png}`)).blob();
      const bitmap = await createImageBitmap(blob);
      const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
      const context = canvas.getContext('2d')!;
      context.drawImage(bitmap, 0, 0);
      const { data } = context.getImageData(0, 0, bitmap.width, bitmap.height);
      let green = 0;
      for (let i = 0; i < data.length; i += 4) {
        if (data[i + 1] > 60 && data[i + 1] > data[i] + 20) green++;
      }
      return green / (data.length / 4);
    }, shot);
  };
  // the grid is grey, only the fern is green
  await expect.poll(lit, { timeout: 5000 }).toBeGreaterThan(0.05);
});

test('angles show in degrees or in radians, the graph keeps radians', async ({ page }) => {
  await open(page);
  await page.evaluate(() =>
    window.nodarium!.graph.load({
      version: 14,
      nodes: [
        { id: 1, type: 'max/plantarium/stem' },
        { id: 2, type: 'max/plantarium/branch', props: { angle: 0.5236 } },
        { id: 3, type: 'max/plantarium/output' }
      ],
      edges: [[1, 0, 2, 'plant'], [2, 0, 3, 'input']]
    }, { title: 'angles' })
  );
  const graph = () => page.evaluate(() => window.nodarium!.graph.get());
  const angle = page.locator(
    'div.wrapper[data-node-type="max/plantarium/branch"][data-node-input="angle"]'
  );
  const field = angle.locator('input');
  await expect(field).toHaveValue('0.5236');
  await expect(angle.getByTestId('unit-label')).toHaveText('rad');
  const before = await graph();

  const chooseAngles = async (name: string) => {
    await page.getByRole('button', { name: 'View', exact: true }).click();
    await page.getByRole('menuitem', { name: 'Angles' }).click();
    await page.getByRole('menuitemcheckbox', { name }).click();
  };

  await chooseAngles('degrees');
  await expect(field).toHaveValue('30');
  await expect(angle.getByTestId('unit-label')).toHaveText('°');
  await expect(angle.locator('label')).toHaveAttribute('title', /in degrees: .*90° along it/);
  expect(await graph()).toEqual(before);

  await field.fill('45');
  await field.press('Enter');
  await expect(field).toHaveValue('45');
  await expect.poll(async () => (await graph()).nodes[1].props?.angle).toBeCloseTo(
    Math.PI / 4,
    12
  );

  await chooseAngles('radians');
  await expect(field).toHaveValue('0.7853981634');
  expect((await graph()).nodes[1].props?.angle).toBeCloseTo(Math.PI / 4, 12);
});
