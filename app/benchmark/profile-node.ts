// Runs one node on inputs `pnpm run profile --dump-args` wrote, without the runtime, through the
// same wrapper and host functions as the runtime. `pnpm profile:node --help` lists the flags,
// they are in PROFILE_NODE_USAGE of args.ts.
import { createWasmWrapper, isWasmSlice } from '@nodarium/utils';
import { existsSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { Session } from 'node:inspector/promises';
import { join } from 'node:path';
import {
  parseProfileNodeArgs,
  PROFILE_NODE_USAGE,
  type ProfileNodeOptions,
  UsageError,
  userPath
} from './args.ts';
import { type CpuProfile, topFunctions } from './cpu-profile.ts';
import { bytes, ms, percent, table } from './format.ts';
import { decodeArgs } from './node-args.ts';
import { median } from './stats.ts';
import { NODES_DIR } from './targets.ts';

const print = (text: string) => process.stdout.write(text + '\n');

const subdirectories = (dir: string) =>
  readdirSync(dir, { withFileTypes: true }).filter(e => e.isDirectory()).map(e => e.name);

/**
 * `max/myzel/strands`, or `strands` in any collection, in a folder like static/nodes
 * (`max/myzel/strands.wasm`) or like the output of cargo (`myzel_strands.wasm`)
 */
function wasmFile(dir: string, node: string) {
  const [user, collection, name] = node.split('/');
  const candidates = name
    ? [join(dir, node + '.wasm'), join(dir, `${collection}_${name}.wasm`)]
    : [
      ...subdirectories(dir).flatMap(u =>
        subdirectories(join(dir, u)).map(c => join(dir, u, c, user + '.wasm'))
      ),
      ...readdirSync(dir).filter(f => f.endsWith(`_${user}.wasm`)).map(f => join(dir, f))
    ];
  const file = candidates.find(existsSync);
  if (!file) throw new UsageError(`there is no node ${node} in ${dir}`);
  return file;
}

async function profileNode(options: ProfileNodeOptions) {
  const file = wasmFile(options.wasm ? userPath(options.wasm) : NODES_DIR, options.node!);
  const argsFile = userPath(options.args!);
  const inputs = decodeArgs(readFileSync(argsFile));
  const wasm = createWasmWrapper(readFileSync(file) as unknown as ArrayBuffer);

  let session: Session | undefined;
  if (options.cpuProf) {
    session = new Session();
    session.connect();
    await session.post('Profiler.enable');
    await session.post('Profiler.setSamplingInterval', { interval: 100 });
    await session.post('Profiler.start');
  }

  const times: number[] = [];
  let outBytes = 0;
  for (let i = 0; i < options.runs; i++) {
    wasm.reset();
    const a = performance.now();
    const result = wasm.execute(inputs);
    times.push(performance.now() - a);
    outBytes = (Array.isArray(result) ? result : [result])
      .reduce((n, r) => n + (isWasmSlice(r) ? r.len * 4 : r.byteLength), 0);
  }

  print(`${file}\n${argsFile}`);
  print(
    `first ${ms(times[0])} ms  median ${ms(median(times))} ms  best ${
      ms(Math.min(...times))
    } ms  in ${bytes(inputs.reduce((n, i) => n + i.byteLength, 0))}  out ${bytes(outBytes)}  wasm ${
      bytes(wasm.memoryBytes())
    }`
  );

  if (session) {
    const { profile } = await session.post('Profiler.stop');
    session.disconnect();
    const out = argsFile.replace(/\.args$/, '') + '.cpuprofile';
    writeFileSync(out, JSON.stringify(profile));
    const { total, top } = topFunctions(profile as CpuProfile, 10);
    print(`\n${out}, ${ms(total)} ms sampled\n`);
    print(table([
      ['self ms', '%', 'function'],
      ...top.map(f => [ms(f.ms), percent(f.share), f.name])
    ], [0, 1]));
  }
}

let options: ProfileNodeOptions;
try {
  options = parseProfileNodeArgs(process.argv.slice(2));
  if (!options.help && (!options.node || !options.args)) {
    throw new UsageError('profile:node takes a node and an args file');
  }
} catch (error) {
  if (!(error instanceof UsageError)) throw error;
  console.error(`${error.message}\n\n${PROFILE_NODE_USAGE}`);
  process.exit(1);
}
if (options.help) {
  print(PROFILE_NODE_USAGE);
  process.exit(0);
}

console.log = console.info = console.error;

try {
  await profileNode(options);
} catch (error) {
  console.error(error instanceof Error ? error.message : error);
  process.exitCode = 1;
}
