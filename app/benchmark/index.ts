import type { Graph, Graph as GraphType, NodeId } from '@nodarium/types';
import { createLogger, createPerformanceStore, splitNestedArray } from '@nodarium/utils';

import { execSync } from 'node:child_process';
import { mkdir, writeFile } from 'node:fs/promises';
import { cpus, freemem, loadavg, totalmem } from 'node:os';
import { resolve } from 'node:path';

import { Group, MeshBasicMaterial } from 'three';
import { createResultPools } from '../src/lib/result-viewer/geometry/geometryPool.ts';
import { MemoryRuntimeCache } from '../src/lib/runtime/runtime-executor-cache.ts';
import { MemoryRuntimeExecutor } from '../src/lib/runtime/runtime-executor.ts';
import { BenchmarkRegistry } from './benchmarkRegistry.ts';

import {
  getMachineInfo,
  measureCpuUsage,
  readCgroupCpuStat,
  readCpuSnapshot,
  readProcMemInfo,
  SystemSample
} from './systemStats.ts';
import champignonTemplate from './templates/champignon-groomed.json' assert { type: 'json' };
import defaultPlantTemplate from './templates/default.json' assert { type: 'json' };
import flyAgaricTemplate from './templates/fly-agaric.json' assert { type: 'json' };
import lottaFacesTemplate from './templates/lotta-faces.json' assert { type: 'json' };
import plantTemplate from './templates/plant.json' assert { type: 'json' };
import porciniTemplate from './templates/porcini.json' assert { type: 'json' };

const SAMPLE_INTERVAL_MS = 200;

const log = createLogger('bench');

type Benchmark = { graph: Graph; runs: number; warmups: number };

const plantRuns = { runs: 100, warmups: 10 };
// a mushroom build takes seconds, fewer runs keep the bench within a few minutes on CI
const mushroomRuns = { runs: 8, warmups: 1 };

const templates: Record<string, Benchmark> = {
  plant: { graph: plantTemplate as unknown as GraphType, ...plantRuns },
  'lotta-faces': { graph: lottaFacesTemplate as unknown as GraphType, ...plantRuns },
  default: { graph: defaultPlantTemplate as unknown as GraphType, ...plantRuns },
  'champignon-groomed': { graph: champignonTemplate as unknown as GraphType, ...mushroomRuns },
  'fly-agaric': { graph: flyAgaricTemplate as unknown as GraphType, ...mushroomRuns },
  porcini: { graph: porciniTemplate as unknown as GraphType, ...mushroomRuns }
};

function git(args: string) {
  try {
    return execSync(`git ${args}`, { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] })
      .trim();
  } catch {
    return null;
  }
}

function getCommitInfo() {
  return {
    sha: process.env.GITHUB_SHA || git('rev-parse HEAD'),
    branch: process.env.GITHUB_HEAD_REF || process.env.GITHUB_REF_NAME
      || git('rev-parse --abbrev-ref HEAD'),
    message: git('log -1 --format=%s')
  };
}

// 0.1μs precision is plenty for timings in ms and keeps the result files small
function roundRuns(data: Record<string, number[]>[]) {
  return data.map(run =>
    Object.fromEntries(
      Object.entries(run).map(([key, values]) => [key, values.map(v => Math.round(v * 1e4) / 1e4)])
    )
  );
}

function average(values: number[]) {
  if (values.length === 0) return 0;
  return values.reduce((a, b) => a + b, 0) / values.length;
}

/**
 * The main thread work of the viewer after a run, without the upload to the GPU. Uncached runs
 * use a random seed, so they measure changed geometry, cached runs measure unchanged geometry.
 */
function createViewerBenchmark() {
  const scene = new Group();
  const material = new MeshBasicMaterial();
  const pools = createResultPools(scene, material);

  return (result: Int32Array) => {
    const a = performance.now();
    const parts = splitNestedArray(result);
    const b = performance.now();
    const { totalVertices, totalFaces } = pools.update(parts);
    const c = performance.now();
    return { split: b - a, update: c - b, totalVertices, totalFaces };
  };
}

/**
 * @param cached - run with the runtime cache and a fixed seed, so every node
 * except the output node is served from the cache after the first run
 */
async function run({ graph: g, runs: amount, warmups }: Benchmark, cached: boolean) {
  // a fresh registry per benchmark, wasm memories never shrink, so shared
  // instances would keep the memory of the largest graph for all benchmarks
  const registry = new BenchmarkRegistry();
  await registry.load(g.nodes.map(n => n.type) as NodeId[]);

  const r = new MemoryRuntimeExecutor(registry, cached ? new MemoryRuntimeCache() : undefined);
  // the template's own settings, like the app passes them: without them every input with a
  // setting falls back to its node's default, the mushrooms to a quarter of their resolution
  const settings = () => ({
    ...g.settings,
    seed: cached ? 0 : Math.floor(Math.random() * 100000000)
  });

  log.log('loaded ' + g.nodes.length + ' nodes');

  const updateViewer = createViewerBenchmark();

  log.log('warming up');

  for (let index = 0; index < warmups; index++) {
    updateViewer(await r.execute(g, settings()));
  }

  const systemSamples: SystemSample[] = [];

  let previousCpuSnapshot = await readCpuSnapshot();

  async function sampleSystem() {
    try {
      const cpu = await measureCpuUsage(previousCpuSnapshot);

      previousCpuSnapshot = cpu.snapshot;

      const [l1, l5, l15] = loadavg();

      systemSamples.push({
        timestamp: Date.now(),

        cpuUsagePercent: cpu.usagePercent,
        cpuStealPercent: cpu.stealPercent,

        load1: l1,
        load5: l5,
        load15: l15,

        freeMemory: freemem(),
        totalMemory: totalmem()
      });
    } catch (err) {
      console.error(err);
    }
  }

  const sampler = setInterval(sampleSystem, SAMPLE_INTERVAL_MS);

  log.log('executing');

  const perfStore = createPerformanceStore();

  r.perf = perfStore;

  let res: Int32Array | undefined;

  const cgroupBefore = await readCgroupCpuStat();
  const processCpuBefore = process.cpuUsage();
  const wallBefore = performance.now();

  for (let i = 0; i < amount; i++) {
    r.perf?.startRun();

    res = await r.execute(g, settings());

    r.perf?.stopRun();

    // not part of the total, which stays the time of the runtime alone
    const viewer = updateViewer(res!);

    r.perf?.addToLastRun('total-vertices', viewer.totalVertices);
    r.perf?.addToLastRun('total-faces', viewer.totalFaces);
    r.perf?.addToLastRun('viewer/split-result', viewer.split);
    r.perf?.addToLastRun('viewer/update-geometries', viewer.update);
  }

  const cgroupAfter = await readCgroupCpuStat();
  const processCpu = process.cpuUsage(processCpuBefore);
  const wall = performance.now() - wallBefore;

  clearInterval(sampler);
  // short benchmarks finish before the first interval
  await sampleSystem();

  log.log('finished');

  // share of the whole machine used by this process, so the dashboard can
  // tell our own load apart from other load on the runner
  const ownCpuPercent = 100 * ((processCpu.user + processCpu.system) / 1000) / wall
    / cpus().length;
  const cpuUsagePercent = average(systemSamples.map(s => s.cpuUsagePercent));

  return {
    data: roundRuns(r.perf.get()),
    metadata: {
      timestamp: new Date().toISOString(),

      commit: getCommitInfo(),

      benchmark: {
        iterations: amount,
        warmups,
        cached
      },

      machine: getMachineInfo(),

      process: {
        pid: process.pid,
        uptime: process.uptime(),

        memoryUsage: process.memoryUsage()
      },

      system: {
        sampleIntervalMs: SAMPLE_INTERVAL_MS,
        sampleCount: systemSamples.length,

        averages: {
          cpuUsagePercent,
          ownCpuPercent,
          otherCpuPercent: Math.max(0, cpuUsagePercent - ownCpuPercent),

          cpuStealPercent: average(
            systemSamples.map(s => s.cpuStealPercent)
          ),

          load1: average(systemSamples.map(s => s.load1)),
          load5: average(systemSamples.map(s => s.load5)),
          load15: average(systemSamples.map(s => s.load15)),

          freeMemory: average(
            systemSamples.map(s => s.freeMemory)
          )
        },

        meminfo: await readProcMemInfo()
      },

      cgroup: {
        before: cgroupBefore,
        after: cgroupAfter
      }
    }
  };
}

async function main() {
  const outPath = resolve('benchmark/out/');

  await mkdir(outPath, { recursive: true });

  for (const key in templates) {
    for (const cached of [false, true]) {
      const name = cached ? key + '-cached' : key;
      log.log('executing ' + name);

      const perfData = await run(templates[key], cached);

      await writeFile(resolve(outPath, name + '.json'), JSON.stringify(perfData));

      await new Promise(res => setTimeout(res, 200));
    }
  }
}

main();
