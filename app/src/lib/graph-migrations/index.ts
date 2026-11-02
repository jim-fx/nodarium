import type { Graph } from '@nodarium/types';
import { createLogger } from '@nodarium/utils';
import { removeInputs, renameInputs } from './helpers';

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
  },
  {
    version: 2,
    description: 'node inputs are named in snake_case, noise always preserves the length',
    migrate(graph) {
      const renames: Record<string, Record<string, string>> = {
        branch: {
          offsetSingle: 'offset_single',
          lowestBranch: 'lowest_branch',
          highestBranch: 'highest_branch'
        },
        instance: { lowestInstance: 'lowest_instance', highestInstance: 'highest_instance' },
        leaf: {
          xResolution: 'x_resolution',
          xCurve: 'x_curve',
          yCurve: 'y_curve',
          yTwist: 'y_twist'
        },
        noise: { fixBottom: 'fix_bottom', directionalStrength: 'directional_strength' },
        snap: { lowestSnap: 'lowest_snap', highestSnap: 'highest_snap' },
        trunk_kink: { lowestKink: 'lowest_kink', highestKink: 'highest_kink' },
        vec3: { 0: 'x', 1: 'y', 2: 'z' }
      };
      for (const [node, inputs] of Object.entries(renames)) {
        renameInputs(graph, `max/plantarium/${node}`, inputs);
      }
      removeInputs(graph, 'max/plantarium/noise', ['preserveLength']);
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
