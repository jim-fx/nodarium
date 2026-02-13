import type { Graph, Graph as GraphType, NodeId } from '@nodarium/types';
import { createLogger, createPerformanceStore } from '@nodarium/utils';
import { mkdir, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { MemoryRuntimeExecutor } from '../src/lib/runtime/runtime-executor.ts';
import { BenchmarkRegistry } from './benchmarkRegistry.ts';
import defaultPlantTemplate from './templates/default.json' assert { type: 'json' };
import lottaFacesTemplate from './templates/lotta-faces.json' assert { type: 'json' };
import plantTemplate from './templates/plant.json' assert { type: 'json' };

const registry = new BenchmarkRegistry();
const r = new MemoryRuntimeExecutor(registry);
const perfStore = createPerformanceStore();

const log = createLogger('bench');

const templates: Record<string, Graph> = {
  'plant': plantTemplate as unknown as GraphType,
  'lotta-faces': lottaFacesTemplate as unknown as GraphType,
  'default': defaultPlantTemplate as unknown as GraphType
};

async function run(g: GraphType, amount: number) {
  await registry.load(plantTemplate.nodes.map(n => n.type) as NodeId[]);
  log.log('loaded ' + g.nodes.length + ' nodes');

  log.log('warming up');

  // Warm up the runtime? maybe this does something?
  for (let index = 0; index < 10; index++) {
    await r.execute(g, { randomSeed: true });
  }

  log.log('executing');
  r.perf = perfStore;
  for (let i = 0; i < amount; i++) {
    r.perf?.startRun();
    await r.execute(g, { randomSeed: true });
    r.perf?.stopRun();
  }
  log.log('finished');
  return r.perf.get();
}

async function main() {
  const outPath = resolve('benchmark/out/');
  await mkdir(outPath, { recursive: true });
  for (const key in templates) {
    log.log('executing ' + key);
    const perfData = await run(templates[key], 100);
    await writeFile(resolve(outPath, key + '.json'), JSON.stringify(perfData));
    await new Promise(res => setTimeout(res, 200));
  }
}

main();
