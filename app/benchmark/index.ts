import type { Graph, Graph as GraphType, NodeId } from '@nodarium/types';
import { createLogger, createPerformanceStore, splitNestedArray } from '@nodarium/utils';

import { execSync } from 'node:child_process';
import { mkdir, writeFile } from 'node:fs/promises';
import { cpus, freemem, loadavg, totalmem } from 'node:os';
import { resolve } from 'node:path';

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
import defaultPlantTemplate from './templates/default.json' assert { type: 'json' };
import lottaFacesTemplate from './templates/lotta-faces.json' assert { type: 'json' };
import plantTemplate from './templates/plant.json' assert { type: 'json' };

const registry = new BenchmarkRegistry();

const SAMPLE_INTERVAL_MS = 200;

const log = createLogger('bench');

const templates: Record<string, Graph> = {
  plant: plantTemplate as unknown as GraphType,
  'lotta-faces': lottaFacesTemplate as unknown as GraphType,
  default: defaultPlantTemplate as unknown as GraphType
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

function countGeometry(result: Int32Array): {
  totalVertices: number;
  totalFaces: number;
} {
  const parts = splitNestedArray(result);

  let totalVertices = 0;
  let totalFaces = 0;

  for (const part of parts) {
    // 0: path, 1: geometry, 2: instances, only the last two are rendered
    const type = part[0];
    if (type !== 1 && type !== 2) continue;

    const vertexCount = part[1] >>> 0;
    const faceCount = part[2] >>> 0;

    if (type === 2) {
      // instance header: [type, vertices, faces, stem_depth, instance amount]
      const instanceCount = part[4] >>> 0;

      totalVertices += vertexCount * instanceCount;
      totalFaces += faceCount * instanceCount;
    } else {
      totalVertices += vertexCount;
      totalFaces += faceCount;
    }
  }

  return {
    totalVertices,
    totalFaces
  };
}

/**
 * @param cached - run with the runtime cache and a fixed seed, so every node
 * except the output node is served from the cache after the first run
 */
async function run(g: GraphType, amount: number, cached: boolean) {
  await registry.load(g.nodes.map(n => n.type) as NodeId[]);

  const r = new MemoryRuntimeExecutor(registry, cached ? new MemoryRuntimeCache() : undefined);
  const settings = { randomSeed: !cached };

  log.log('loaded ' + g.nodes.length + ' nodes');

  log.log('warming up');

  for (let index = 0; index < 10; index++) {
    await r.execute(g, settings);
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

    res = await r.execute(g, settings);

    r.perf?.stopRun();

    const { totalVertices, totalFaces } = countGeometry(res!);

    r.perf?.addToLastRun('total-vertices', totalVertices);
    r.perf?.addToLastRun('total-faces', totalFaces);
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

      const perfData = await run(templates[key], 100, cached);

      await writeFile(resolve(outPath, name + '.json'), JSON.stringify(perfData));

      await new Promise(res => setTimeout(res, 200));
    }
  }
}

main();
