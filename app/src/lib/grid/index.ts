import { withSubComponents } from '#lib/helpers/index.ts';
import Cell from './Cell.svelte';
import Grid from './Grid.svelte';
import Row from './Row.svelte';

export default withSubComponents(Grid, { Row, Cell });
