import type { Pointer } from '$lib/runtime';

export function copyVisibleMemory(rows: Int32Array, currentPtrs: Pointer[], start: number) {
  if (!rows?.length) return;

  // Build an array of rows for the table
  const tableRows = [...rows].map((value, i) => {
    const index = start + i;
    const ptr = currentPtrs[i];
    return {
      index,
      ptr: ptr?._title ?? '',
      value: value
    };
  });

  // Compute column widths
  const indexWidth = Math.max(
    5,
    ...tableRows.map((r) => r.index.toString().length)
  );
  const ptrWidth = Math.max(
    10,
    ...tableRows.map((r) => r.ptr.length)
  );
  const valueWidth = Math.max(
    10,
    ...tableRows.map((r) => r.value.toString().length)
  );

  // Build header
  let output =
    `| ${'Index'.padEnd(indexWidth)} | ${'Ptr'.padEnd(ptrWidth)} | ${'Value'.padEnd(valueWidth)
    } |\n`
    + `|-${'-'.repeat(indexWidth)}-|-${'-'.repeat(ptrWidth)}-|-${'-'.repeat(valueWidth)}-|\n`;

  // Add rows
  for (const row of tableRows) {
    output += `| ${row.index.toString().padEnd(indexWidth)} | ${row.ptr.padEnd(ptrWidth)} | ${row.value.toString().padEnd(valueWidth)
      } |\n`;
  }

  // Copy to clipboard
  navigator.clipboard
    .writeText(output)
    .then(() => console.log('Memory + metadata copied as table'))
    .catch((err) => console.error('Failed to copy memory:', err));
}
