import { CatmullRomCurve3, Color, Euler, Matrix4, Quaternion, Vector3 } from 'three';
import { BED_DEPTH, BED_WIDTH, PLANT_ISLAND_RADIUS } from '../config';
import { createRandom } from '../random';
import { GROUND_LEVEL, PATH_POINTS, PATH_WIDTH } from './config';

export interface Instances {
  matrices: Matrix4[];
  colors: Color[];
}

function instances(): Instances {
  return { matrices: [], colors: [] };
}

type ClumpKind = 'rose' | 'white' | 'yellow' | 'foliage';

interface Clump {
  x: number;
  z: number;
  /** height of the ground the clump grows on */
  y: number;
  radius: number;
  height: number;
  kind: ClumpKind;
}

/** Leaf colors per kind of clump, from shadowy to sunlit */
const LEAVES: Record<ClumpKind, [string, string]> = {
  rose: ['#2f5a22', '#5d8c3a'],
  white: ['#3f6e2a', '#7aa84c'],
  yellow: ['#8aa82a', '#cddc4a'],
  foliage: ['#365f24', '#86b04e']
};

const FLOWERS = {
  rose: ['#b3121b', '#d42a2a', '#e04f6a', '#c21d3a'],
  white: ['#f4f6f2', '#ffffff', '#e8eef8'],
  yellow: ['#f2c21a', '#f7d43a', '#e8b010'],
  allium: ['#8a55c4', '#9d68d2', '#b07ce0', '#7a48b0']
};

const up = new Vector3(0, 1, 0);
const scratch = {
  direction: new Vector3(),
  position: new Vector3(),
  rotation: new Quaternion(),
  twist: new Quaternion(),
  scale: new Vector3()
};

export function createPathCurve() {
  return new CatmullRomCurve3(
    PATH_POINTS.map(([x, z]) => new Vector3(x, 0, z)),
    false,
    'centripetal'
  );
}

/** Distance from a point on the ground to the center line of the path */
export function createPathDistance() {
  const points = createPathCurve().getSpacedPoints(120);
  return (x: number, z: number) => {
    let best = Infinity;
    for (let i = 1; i < points.length; i++) {
      const a = points[i - 1], b = points[i];
      const dx = b.x - a.x, dz = b.z - a.z;
      const t = Math.max(0, Math.min(1, ((x - a.x) * dx + (z - a.z) * dz) / (dx * dx + dz * dz)));
      best = Math.min(best, Math.hypot(x - (a.x + dx * t), z - (a.z + dz * t)));
    }
    return best;
  };
}

/** Distance to the raised bed, 0 inside of it */
function bedDistance(x: number, z: number) {
  const dx = Math.max(Math.abs(x) - BED_WIDTH / 2 - 0.35, 0);
  const dz = Math.max(Math.abs(z) - BED_DEPTH / 2 - 0.35, 0);
  return Math.hypot(dx, dz);
}

/** Point on the visible part of an upright ellipsoid, y going from 0 at the rim to 1 on top */
function onDome(random: () => number, target: Vector3) {
  const angle = random() * Math.PI * 2;
  const y = Math.pow(random(), 0.75);
  const r = Math.sqrt(1 - y * y);
  return target.set(Math.cos(angle) * r, y, Math.sin(angle) * r);
}

function pick<T>(random: () => number, list: T[]) {
  return list[Math.floor(random() * list.length)];
}

function placeClumps(plants: { x: number; z: number }[]) {
  const random = createRandom(21);
  const pathDistance = createPathDistance();
  const clumps: Clump[] = [];

  const fits = (c: Clump) =>
    clumps.every((o) => Math.hypot(o.x - c.x, o.z - c.z) > (o.radius + c.radius) * 0.7);

  // lush borders on both sides of the path and around the raised bed
  for (let i = 0; clumps.length < 120 && i < 4000; i++) {
    const x = (random() * 2 - 1) * 17;
    const z = -8.5 + random() * 24;
    const bed = bedDistance(x, z);
    const path = pathDistance(x, z) - PATH_WIDTH / 2;
    // the borders hug the path and the bed, further away the lawn takes over
    const near = Math.min(bed < 0.2 ? Infinity : bed, path);
    const radius = 0.6 + random() * 1.1;
    // clumps may lean over the edges of the path, but must not cover it
    if (bed < 0.2 || path < radius * 0.55 || near > 5 + random() * 2) continue;
    const clump: Clump = {
      x,
      z,
      y: GROUND_LEVEL,
      radius,
      height: radius * (0.5 + random() * 0.5),
      kind: pick(random, ['rose', 'rose', 'white', 'yellow', 'foliage', 'foliage'])
    };
    if (fits(clump)) clumps.push(clump);
  }

  // a few patches of low ground cover between the plants of the raised bed
  const borders = clumps.length;
  for (let i = 0; clumps.length < borders + 30 && i < 600; i++) {
    const x = (random() * 2 - 1) * (BED_WIDTH / 2 - 0.5);
    const z = (random() * 2 - 1) * (BED_DEPTH / 2 - 0.5);
    const radius = 0.4 + random() * 0.5;
    const free = plants.every((p) =>
      Math.hypot(p.x - x, p.z - z) > PLANT_ISLAND_RADIUS + radius * 0.6
    )
      && clumps.slice(borders).every((o) => Math.hypot(o.x - x, o.z - z) > 1.3);
    const clump: Clump = {
      x,
      z,
      y: 0,
      radius,
      height: 0.18 + random() * 0.18,
      kind: pick(random, ['white', 'yellow', 'yellow', 'foliage'])
    };
    if (free && fits(clump)) clumps.push(clump);
  }

  return clumps;
}

/** Leaves and flowers of all clumps */
function growClumps(clumps: Clump[]) {
  const random = createRandom(33);
  const leaves = instances();
  const roses = instances();
  const florets = instances();
  const { direction, position, rotation, twist, scale } = scratch;
  const color = new Color();
  const tip = new Color();

  for (const clump of clumps) {
    const [dark, light] = LEAVES[clump.kind];
    const leafCount = Math.min(Math.round(170 * clump.radius * clump.radius + 30), 520);
    const leafSize = clump.y === 0 ? 0.13 : 0.2;
    for (let i = 0; i < leafCount; i++) {
      onDome(random, direction);
      const depth = 0.55 + random() * 0.45;
      position.set(
        clump.x + direction.x * clump.radius * depth,
        clump.y + direction.y * clump.height * depth,
        clump.z + direction.z * clump.radius * depth
      );
      // leaves grow outwards and up, each one twisted a little differently
      direction.y += 0.5;
      direction.x += (random() - 0.5) * 0.6;
      direction.z += (random() - 0.5) * 0.6;
      rotation.setFromUnitVectors(up, direction.normalize());
      rotation.multiply(twist.setFromAxisAngle(up, random() * Math.PI * 2));
      const size = leafSize * (0.7 + random() * 0.6);
      leaves.matrices.push(new Matrix4().compose(position, rotation, scale.setScalar(size)));
      // leaves deep inside the clump are in its own shadow
      color.set(dark).lerp(tip.set(light), random() * 0.8 * depth + 0.2 * direction.y);
      leaves.colors.push(color.clone());
    }

    if (clump.kind === 'foliage') continue;
    const flowerCount = Math.round(
      (clump.kind === 'rose' ? 9 : 55) * clump.radius * clump.radius * (clump.y === 0 ? 0.6 : 1)
    );
    for (let i = 0; i < flowerCount; i++) {
      onDome(random, direction);
      if (direction.y < 0.35) direction.y = 0.35 + random() * 0.3;
      position.set(
        clump.x + direction.x * clump.radius * 0.95,
        clump.y + direction.y * clump.height * 1.02,
        clump.z + direction.z * clump.radius * 0.95
      );
      direction.y += 1.2;
      rotation.setFromUnitVectors(up, direction.normalize());
      rotation.multiply(twist.setFromAxisAngle(up, random() * Math.PI * 2));
      const isRose = clump.kind === 'rose';
      const size = isRose
        ? 0.26 + random() * 0.12
        : (clump.y === 0 ? 0.05 : 0.065) * (0.8 + random() * 0.4);
      const target = isRose ? roses : florets;
      target.matrices.push(new Matrix4().compose(position, rotation, scale.setScalar(size)));
      target.colors.push(new Color(pick(random, FLOWERS[clump.kind])));
    }
  }

  return { leaves, roses, florets };
}

/** Clusters of tall alliums that rise above the borders */
function growAlliums(clumps: Clump[]) {
  const random = createRandom(57);
  const stems = instances();
  const heads = instances();
  const { position, rotation, scale } = scratch;
  const tilt = new Euler();
  const borders = clumps.filter((c) => c.y !== 0 && c.kind !== 'yellow');
  // some stand close to the visitor, far out of focus, but never in front of the bed
  const front = borders.filter((c) =>
    c.z > BED_DEPTH / 2 + 1 && (Math.abs(c.x) > BED_WIDTH / 2 - 1 || c.z > BED_DEPTH / 2 + 7)
  );

  for (let cluster = 0; cluster < 11; cluster++) {
    const center = pick(random, cluster % 2 ? front : borders);
    const count = 3 + Math.floor(random() * 6);
    for (let i = 0; i < count; i++) {
      const height = 2 + random() * 1.8;
      position.set(
        center.x + (random() - 0.5) * center.radius * 1.4,
        GROUND_LEVEL,
        center.z + (random() - 0.5) * center.radius * 1.4
      );
      tilt.set((random() - 0.5) * 0.25, 0, (random() - 0.5) * 0.25);
      rotation.setFromEuler(tilt);
      stems.matrices.push(
        new Matrix4().compose(position, rotation, scale.set(0.022, height, 0.022))
      );
      stems.colors.push(new Color('#5f8a3a').offsetHSL(0, 0, (random() - 0.5) * 0.08));

      const top = new Vector3(0, height, 0).applyQuaternion(rotation).add(position);
      const size = 0.22 + random() * 0.1;
      heads.matrices.push(new Matrix4().compose(top, rotation, scale.setScalar(size)));
      heads.colors.push(new Color(pick(random, FLOWERS.allium)));
    }
  }
  return { stems, heads };
}

/** Grass blades for the lawn around everything else */
function growLawn(clumps: Clump[], count: number) {
  const random = createRandom(71);
  const blades = instances();
  const pathDistance = createPathDistance();
  const { position, rotation, twist, scale } = scratch;
  const dark = new Color('#3d5a1e');
  const light = new Color('#8fae4a');
  const dry = new Color('#b2ab62');
  const borders = clumps.filter((c) => c.y !== 0);

  for (let i = 0; blades.matrices.length < count && i < count * 4; i++) {
    const x = (random() * 2 - 1) * 26;
    // more blades close to the visitor, where they can be seen
    const z = -9.5 + Math.pow(random(), 0.7) * 30;
    if (bedDistance(x, z) < 0.1 || pathDistance(x, z) < PATH_WIDTH / 2 + 0.05) continue;
    if (borders.some((c) => Math.hypot(c.x - x, c.z - z) < c.radius * 0.75)) continue;

    position.set(x, GROUND_LEVEL, z);
    rotation.setFromAxisAngle(up, random() * Math.PI * 2);
    rotation.multiply(twist.setFromAxisAngle(new Vector3(1, 0, 0), (random() - 0.5) * 0.5));
    const height = 0.25 + random() * 0.35;
    blades.matrices.push(
      new Matrix4().compose(position, rotation, scale.set(1 + random(), height, 1))
    );
    const color = dark.clone().lerp(light, random());
    if (random() < 0.08) color.lerp(dry, 0.6);
    blades.colors.push(color);
  }
  return blades;
}

/** Everything that grows in the cottage garden apart from the projects */
export function growFlora(plants: { x: number; z: number }[], grassCount: number) {
  const clumps = placeClumps(plants);
  return {
    ...growClumps(clumps),
    ...growAlliums(clumps),
    grass: growLawn(clumps, grassCount)
  };
}
