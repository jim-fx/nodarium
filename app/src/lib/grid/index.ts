import { withSubComponents } from '$lib/helpers';
import Cell from './Cell.svelte';
import Grid from './Grid.svelte';
import Row from './Row.svelte';

export default withSubComponents(Grid, { Row, Cell });
