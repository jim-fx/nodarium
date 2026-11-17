import { readFileSync } from 'node:fs';

/** docs/GRAPHS.md of the repository, the build puts its text in the place of this module */
export const GRAPHS_DOC = readFileSync(
  new URL('../../../../docs/GRAPHS.md', import.meta.url),
  'utf8'
);
