import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { page } from 'vitest/browser';
import { unitSettings } from '../units.svelte';
import InputNumber from './InputNumber.svelte';

describe('InputNumber', () => {
  it('should render input element', async () => {
    render(InputNumber, { value: 0.5 });

    const input = page.getByRole('spinbutton');
    await expect.element(input).toBeInTheDocument();
  });

  it('should render with step buttons when step is provided', async () => {
    render(InputNumber, { value: 0.5, step: 0.1 });

    const decrementBtn = page.getByRole('button', { name: 'step down' });
    const incrementBtn = page.getByRole('button', { name: 'step up' });
    await expect.element(decrementBtn).toBeInTheDocument();
    await expect.element(incrementBtn).toBeInTheDocument();
  });

  it('should not render step buttons when step is undefined', async () => {
    const screen = await render(InputNumber, { value: 0.5 });

    const buttons = screen.locator.getByRole('button');
    const count = buttons.all().length;
    expect(count).toBe(0);
  });

  it('should accept numeric value', async () => {
    render(InputNumber, { value: 42 });

    const input = page.getByRole('spinbutton');
    await expect.element(input).toHaveValue(42);
  });

  it('should accept min and max bounds', async () => {
    render(InputNumber, { value: 5, min: 0, max: 10 });

    const input = page.getByRole('spinbutton');
    await expect.element(input).toHaveAttribute('min', '0');
    await expect.element(input).toHaveAttribute('max', '10');
  });

  it('should not clamp value on init when value exceeds min/max', async () => {
    render(InputNumber, { value: 100, min: 0, max: 10 });

    const input = page.getByRole('spinbutton');
    await expect.element(input).toHaveValue(100);
  });

  it('should not snap value to the step on init', async () => {
    render(InputNumber, { value: 0.002, step: 0.05 });

    const input = page.getByRole('spinbutton');
    await expect.element(input).toHaveValue(0.002);
  });

  it('should not clamp value on init when value is below min', async () => {
    render(InputNumber, { value: -50, min: 0, max: 10 });

    const input = page.getByRole('spinbutton');
    await expect.element(input).toHaveValue(-50);
  });

  it('should show a length in the project unit', async () => {
    unitSettings.unit = 'cm';
    try {
      render(InputNumber, { value: 0.25, unit: 'm' });
      await expect.element(page.getByRole('textbox')).toHaveValue('25');
      await expect.element(page.getByTestId('unit-label')).toHaveTextContent('cm');
    } finally {
      unitSettings.unit = 'm';
    }
  });

  it('should read typed units', async () => {
    unitSettings.unit = 'cm';
    try {
      const screen = render(InputNumber, { value: 0.25, unit: 'm' });
      const input = page.getByRole('textbox');
      await input.fill('1m');
      await input.element().blur();
      await expect.element(input).toHaveValue('100');
      await input.fill('50mm');
      await input.element().blur();
      await expect.element(input).toHaveValue('5');
      await input.fill('12');
      await input.element().blur();
      await expect.element(input).toHaveValue('12');
      await input.fill('abc');
      await input.element().blur();
      await expect.element(input).toHaveValue('12');
      void screen;
    } finally {
      unitSettings.unit = 'm';
    }
  });

  it('should not convert or label inputs without a unit', async () => {
    unitSettings.unit = 'cm';
    try {
      render(InputNumber, { value: 0.25 });
      await expect.element(page.getByRole('spinbutton')).toHaveValue(0.25);
      await expect.element(page.getByTestId('unit-label')).not.toBeInTheDocument();
    } finally {
      unitSettings.unit = 'm';
    }
  });
});
