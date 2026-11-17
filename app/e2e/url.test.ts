import { encodeGraph } from '@nodarium/runtime';
import { expect, type Page, test } from '@playwright/test';

test.use({ viewport: { width: 1920, height: 1080 } });

const APP = 'http://localhost:4173/';

const plant = {
  version: 13,
  meta: { title: 'Linked plant' },
  nodes: [
    { id: 1, type: 'max/plantarium/stem', props: { amount: 3, length: 0.5 } },
    { id: 2, type: 'max/plantarium/output' }
  ],
  edges: [[1, 0, 2, 'input']]
};

async function open(page: Page, url: string) {
  await page.goto(url);
  await page.waitForFunction(() => window.nodarium, null, { polling: 100 });
  return page.evaluate(async () => {
    const nodarium = window.nodarium!;
    await nodarium.ready;
    const graph = nodarium.graph.get();
    return {
      id: graph.id,
      title: graph.meta?.title,
      seed: graph.settings?.seed,
      result: nodarium.lastResult(),
      address: location.pathname + location.search + location.hash
    };
  });
}

function countProjects(page: Page) {
  return page.evaluate(() =>
    new Promise<number>((resolve, reject) => {
      const request = indexedDB.open('nodarium-graphs');
      request.onerror = () => reject(request.error);
      request.onsuccess = () => {
        const count = request.result.transaction('graphs').objectStore('graphs').count();
        count.onsuccess = () => {
          request.result.close();
          resolve(count.result);
        };
      };
    })
  );
}

test('?template= opens the template with the seed and the panel, then leaves the address', async ({ page }) => {
  const opened = await open(page, `${APP}?template=fern&seed=7&panel=graph-settings&tutorial=0`);
  expect(opened).toMatchObject({ title: 'Fern', seed: 7, address: '/' });
  expect(opened.result?.ok).toBe(true);
  expect(opened.result?.faces).toBeGreaterThan(1000);
  expect(await page.evaluate(() => localStorage.getItem('node.activePanel'))).toBe(
    '"graph-settings"'
  );

  const projects = await countProjects(page);
  const again = await open(page, `${APP}?template=fern&seed=7&tutorial=0`);
  expect(again.id).toBe(opened.id);
  expect(await countProjects(page)).toBe(projects);
});

test('a template with groups opened twice is one project', async ({ page }) => {
  const first = await open(page, `${APP}?template=oyster&tutorial=0`);
  const projects = await countProjects(page);
  const second = await open(page, `${APP}?template=oyster&tutorial=0`);
  expect(second.id).toBe(first.id);
  expect(await countProjects(page)).toBe(projects);
});

test('#graph= opens the graph of the link once, a second visit opens the same project', async ({ page }) => {
  const link = `${APP}?tutorial=0#graph=${await encodeGraph(plant)}`;
  const first = await open(page, link);
  expect(first).toMatchObject({ title: 'Linked plant', address: '/' });
  expect(first.result?.ok).toBe(true);
  const projects = await countProjects(page);

  const second = await open(page, link);
  expect(second.id).toBe(first.id);
  expect(await countProjects(page)).toBe(projects);

  const seeded = await open(page, `${APP}?seed=3&tutorial=0#graph=${await encodeGraph(plant)}`);
  expect(seeded.id).not.toBe(first.id);
  expect(seeded.seed).toBe(3);
  expect(await countProjects(page)).toBe(projects + 1);
});

test('#graph= opens in a tab that has the app open, where only the fragment changes', async ({ page }) => {
  const before = await open(page, `${APP}?tutorial=0`);
  await page.goto(`${APP}#graph=${await encodeGraph(plant)}`);
  await page.waitForFunction(
    () => window.nodarium!.graph.get().meta?.title === 'Linked plant',
    null,
    { polling: 100 }
  );
  const opened = await page.evaluate(async () => {
    await window.nodarium!.idle();
    return {
      id: window.nodarium!.graph.get().id,
      ok: window.nodarium!.lastResult()?.ok,
      address: location.pathname + location.search + location.hash
    };
  });
  expect(opened).toMatchObject({ ok: true, address: '/' });
  expect(opened.id).not.toBe(before.id);
});

test('a broken link says why and opens the app as it was', async ({ page }) => {
  const before = await open(page, `${APP}?tutorial=0`);
  const projects = await countProjects(page);
  const data = await encodeGraph(plant);

  const broken = await open(page, `${APP}?tutorial=0#graph=${data.slice(0, 40)}`);
  await expect(page.getByText('Opening the link failed: the graph in the link is broken'))
    .toBeVisible();
  expect(broken).toMatchObject({ id: before.id, address: '/' });
  expect(await countProjects(page)).toBe(projects);

  await open(page, `${APP}?template=nope&tutorial=0`);
  await expect(page.getByText('Opening the link failed: there is no template "nope"'))
    .toBeVisible();
  expect(await countProjects(page)).toBe(projects);
});

test('File → Copy link copies a link that opens the project', async ({ page }) => {
  await page.addInitScript(() => {
    navigator.clipboard.writeText = async (text) => {
      (window as unknown as { copied: string }).copied = text;
    };
  });
  const opened = await open(page, `${APP}?template=daisy&tutorial=0`);
  await page.getByRole('button', { name: 'File', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Copy link' }).click();
  await expect(page.getByText('Link copied to clipboard')).toBeVisible();
  const link = await page.evaluate(() => (window as unknown as { copied: string }).copied);
  expect(link).toMatch(/^http:\/\/localhost:4173\/#graph=[\w-]+$/);

  const projects = await countProjects(page);
  const reopened = await open(page, link);
  expect(reopened).toMatchObject({ id: opened.id, title: 'Daisy' });
  expect(await countProjects(page)).toBe(projects);
});
