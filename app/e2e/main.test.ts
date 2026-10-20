import { expect, test } from '@playwright/test';

// nodes outside of the camera are not rendered, the default plant needs the room
test.use({ viewport: { width: 1920, height: 1080 } });

test('test', async ({ page }) => {
  // Listen for console messages
  page.on('console', msg => {
    console.log(`[Browser Console] ${msg.type()}: ${msg.text()}`);
  });

  // keep the tutorial from auto-starting on the first visit
  await page.addInitScript(() => {
    localStorage.setItem('planty:nodarium-tutorial:seen', 'e2e');
  });

  await page.goto('http://localhost:4173', { waitUntil: 'load' });

  await page.getByRole('button', { name: 'File', exact: true }).click();
  await page.getByRole('menuitem', { name: /^New project/ }).click();
  const projectBrowser = page.getByRole('dialog', { name: 'Projects' });
  await projectBrowser.getByRole('button', { name: 'Default Plant', exact: true }).click();
  await expect(projectBrowser).toBeHidden();

  const expectedNodes = [
    {
      id: '10',
      type: 'max/plantarium/stem',
      props: {
        amount: 6,
        length: 11,
        thickness: 0.1
      }
    },
    {
      id: '15',
      type: 'max/plantarium/noise',
      props: {
        scale: 2.2,
        strength: 4.9
      }
    },
    {
      id: '9',
      type: 'max/plantarium/output'
    }
  ];

  for (const node of expectedNodes) {
    const wrapper = page.locator(
      `div.wrapper[data-node-id="${node.id}"][data-node-type="${node.type}"]`
    );
    await expect(wrapper).toBeVisible();
    if ('props' in node) {
      const props = node.props as unknown as Record<string, number>;
      for (const propId in node.props) {
        const expectedValue = props[propId];
        const inputElement = page.locator(
          `div.wrapper[data-node-type="${node.type}"][data-node-input="${propId}"] input[type="number"]`
        );
        const value = parseFloat(await inputElement.inputValue());
        expect(value).toBe(expectedValue);
      }
    }
  }
});
