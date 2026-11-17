import { INTERNAL_PREFIX, type NodeDefinition, type NodeId } from '@nodarium/types';
import { readdir } from 'node:fs/promises';
import { join } from 'node:path';
import { nodesDir } from './nodes-dir';
import { FileNodeRegistry } from './registry/file-node-registry';

async function subdirectories(dir: string) {
  const entries = await readdir(dir, { withFileTypes: true }).catch(() => []);
  return entries.filter((entry) => entry.isDirectory()).map((entry) => entry.name);
}

/** The ids of the nodes in `dir`, from the files `<user>/<collection>/<node>.wasm` */
export async function listNodeIds(dir: string = nodesDir): Promise<NodeId[]> {
  const ids: NodeId[] = [];
  for (const user of await subdirectories(dir)) {
    for (const collection of await subdirectories(join(dir, user))) {
      const files = await readdir(join(dir, user, collection));
      for (const file of files.filter((f) => f.endsWith('.wasm'))) {
        ids.push(`${user}/${collection}/${file.slice(0, -5)}` as NodeId);
      }
    }
  }
  return ids.sort();
}

/**
 * The definitions of all nodes in `dir`, the nodes of the package by default, sorted by their
 * id. Every node is compiled to read its definition, `registry` keeps them for a run.
 */
export async function listNodes(
  dir: string = nodesDir,
  registry = new FileNodeRegistry(dir)
): Promise<NodeDefinition[]> {
  const ids = await listNodeIds(dir);
  if (ids.length === 0) throw new Error(`No nodes in ${dir}`);
  await registry.load(ids);
  return registry.getAllNodes()
    .filter((node) => !node.id.startsWith(INTERNAL_PREFIX))
    .sort((a, b) => a.id.localeCompare(b.id));
}
