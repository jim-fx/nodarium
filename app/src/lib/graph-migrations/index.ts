import type { Graph } from '@nodarium/types';
import { createLogger } from '@nodarium/utils';

export * from './helpers';

const log = createLogger('graph-migrations');

export type Migration = {
  version: number;
  description: string;
  migrate(graph: Graph): Graph;
};

// Only append, never change a released migration
export const MIGRATIONS: Migration[] = [
  {
    version: 1,
    description: 'graphs carry a version, graphs from before groups get an empty list of them',
    migrate(graph) {
      graph.groups ??= [];
      return graph;
    }
  }
];

export const CURRENT_GRAPH_VERSION = MIGRATIONS.at(-1)?.version ?? 0;

export class GraphVersionError extends Error {
  constructor(public readonly graphVersion: number) {
    super(
      `This graph was saved by a newer version of Nodarium (graph version ${graphVersion}, `
        + `this app knows up to ${CURRENT_GRAPH_VERSION}). Reload the page to update the app.`
    );
    this.name = 'GraphVersionError';
  }
}

export function migrateGraph(graph: Graph): Graph {
  const from = graph.version ?? 0;
  if (from > CURRENT_GRAPH_VERSION) throw new GraphVersionError(from);
  if (from === CURRENT_GRAPH_VERSION) return graph;

  // structuredClone can't copy svelte proxies
  let migrated: Graph = JSON.parse(JSON.stringify(graph));
  const applied = MIGRATIONS.filter((m) => m.version > from);
  for (const migration of applied) {
    migrated = migration.migrate(migrated);
    migrated.version = migration.version;
  }

  log.info(
    `migrated graph ${graph.id} from version ${from} to ${CURRENT_GRAPH_VERSION}`,
    applied.map((m) => `${m.version}: ${m.description}`)
  );
  return migrated;
}
