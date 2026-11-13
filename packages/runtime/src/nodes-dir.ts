import { fileURLToPath } from 'node:url';

// this module sits one level below the package root, as src/nodes-dir.ts and bundled in dist/
export const nodesDir = fileURLToPath(new URL('../nodes/', import.meta.url));
