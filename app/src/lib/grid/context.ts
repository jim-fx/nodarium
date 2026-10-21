import { createContext } from 'svelte';

export const [getGridId, setGridId, hasGridId] = createContext<string | undefined>();

export type GridRowContext = {
  /** Reserves the next column of the row and returns its index */
  registerCell: () => number;
  sizes: { value: string[] };
};

export const [getGridRow, setGridRow] = createContext<GridRowContext>();
