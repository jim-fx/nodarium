import type { Graph } from '@nodarium/types';
import { createLogger } from '@nodarium/utils';
import { capAtPoints } from './cap-at-points';
import { removeInputs, renameInputs, replaceNodes, wiredInputs } from './helpers';
import { joinMeshes } from './join-meshes';
import { lamellaeRidges } from './lamellae-ridges';
import { lopsidedByRadius } from './lopsided-by-radius';
import { marginCurveRandom } from './margin-curve-random';
import { membraneSkirts } from './membrane-skirts';
import { outputMesh } from './output-mesh';
import { setMeshColor } from './set-mesh-color';
import { splitCap, wiredCapInputs } from './split-cap';
import { STRANDS, strandsThickness } from './strands-thickness';
import { strandsTubeOnly } from './strands-tube-only';

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
  },
  {
    version: 3,
    description:
      'the cap only grows the open cap, fill_gaps, margin_waves, margin_curl and cap_open pose it',
    migrate(graph) {
      const wired = wiredCapInputs(graph);
      return replaceNodes(
        graph,
        'max/myzel/cap',
        (old, context) => splitCap(old, context, wired.get(old))
      );
    }
  },
  {
    version: 4,
    description: 'strands lost thickness_variation, a random node into the thickness varies it',
    migrate(graph) {
      const wired = wiredInputs(graph, STRANDS);
      replaceNodes(
        graph,
        STRANDS,
        (old, context) => strandsThickness(old, context, wired.get(old))
      );
      return removeInputs(graph, STRANDS, ['thickness_variation']);
    }
  },
  {
    version: 5,
    description:
      'hyphae_to_mesh takes paths only, join_meshes joins its mesh with those of lamellae and membrane',
    migrate: joinMeshes
  },
  {
    version: 6,
    description:
      'set_color takes paths only, its mesh moved to set_mesh_color and the input plant is called paths',
    migrate: setMeshColor
  },
  {
    version: 7,
    description:
      'the Myzel output takes a mesh and shows paths as lines, hyphae_to_mesh makes the surface of paths',
    migrate: outputMesh
  },
  {
    version: 8,
    description:
      'the cap grows at points and gives its stipe with the rays, the pose nodes find the cap from the stipe and cap_margin gives the margin',
    migrate: capAtPoints
  },
  {
    version: 9,
    description:
      'cap_lopsided is gone, a curve into the cap radius makes a cap lopsided, random nodes into the radius draw once per path',
    migrate: lopsidedByRadius
  },
  {
    version: 10,
    description:
      'strands only fills the tube, a twist node turns the strands and a random_break behind it ends some of them short of the tip',
    migrate: strandsTubeOnly
  },
  {
    version: 11,
    description:
      'lamellae reads its depth along the gill and lost depth_curve and blunt, a curve node into the depth shapes it and ridges makes the blunt folds',
    migrate: lamellaeRidges
  },
  {
    version: 12,
    description:
      'margin_curl lost reach and margin_waves lost irregularity and seed, the amount is read along the ray and the amplitude for every lobe, a curve and a random node shape them',
    migrate: marginCurveRandom
  },
  {
    version: 13,
    description:
      'membrane is the closed skin and wraps around a point in b, a points_on_path places it, and skirt nodes hang the ring and the fringe once it has torn',
    migrate: membraneSkirts
  }
];

export const CURRENT_GRAPH_VERSION = MIGRATIONS.at(-1)?.version ?? 0;

export class GraphVersionError extends Error {
  constructor(public readonly graphVersion: number) {
    super(
      `This graph has version ${graphVersion}, the highest version this runtime knows is `
        + `${CURRENT_GRAPH_VERSION}. Update @nodarium/runtime to run it.`
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
