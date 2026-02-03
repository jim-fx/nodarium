import { expect, test } from '@playwright/test';

test('test', async ({ page }) => {
  // Listen for console messages
  page.on('console', msg => {
    console.log(`[Browser Console] ${msg.type()}: ${msg.text()}`);
  });

  await page.goto('http://localhost:4173', { waitUntil: 'load' });

  // await expect(page).toHaveScreenshot();
  await expect(page.locator('.graph-wrapper')).toHaveScreenshot();

  await page.getByRole('button', { name: 'projects' }).click();
  await page.getByRole('button', { name: 'New', exact: true }).click();
  await page.getByRole('combobox').selectOption('2');
  await page.getByRole('textbox', { name: 'Project name' }).click();
  await page.getByRole('textbox', { name: 'Project name' }).fill('Test Project');
  await page.getByRole('button', { name: 'Create' }).click();

  const expectedNodes = [
    {
      id: '10',
      type: 'max/plantarium/stem',
      props: {
        amount: 50,
        length: 4,
        thickness: 1
      }
    },
    {
      id: '11',
      type: 'max/plantarium/noise',
      props: {
        scale: 0.5,
        strength: 5
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
