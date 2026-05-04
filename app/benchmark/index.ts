import type { Graph, Graph as GraphType, NodeId } from '@nodarium/types';
import { createLogger, createPerformanceStore, splitNestedArray } from '@nodarium/utils';

import { mkdir, writeFile } from 'node:fs/promises';
import { freemem, loadavg, totalmem } from 'node:os';
import { resolve } from 'node:path';

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
const r = new MemoryRuntimeExecutor(registry);

const log = createLogger('bench');

const templates: Record<string, Graph> = {
  plant: plantTemplate as unknown as GraphType,
  'lotta-faces': lottaFacesTemplate as unknown as GraphType,
  default: defaultPlantTemplate as unknown as GraphType
};

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
    const type = part[0];

    const vertexCount = part[1] >>> 0;
    const faceCount = part[2] >>> 0;

    if (type === 2) {
      const instanceCount = part[3] >>> 0;

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

async function run(g: GraphType, amount: number) {
  await registry.load(g.nodes.map(n => n.type) as NodeId[]);

  log.log('loaded ' + g.nodes.length + ' nodes');

  log.log('warming up');

  for (let index = 0; index < 10; index++) {
    await r.execute(g, { randomSeed: true });
  }

  const systemSamples: SystemSample[] = [];

  let previousCpuSnapshot = await readCpuSnapshot();

  const sampler = setInterval(async () => {
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
  }, 1000);

  log.log('executing');

  const perfStore = createPerformanceStore();

  r.perf = perfStore;

  let res: Int32Array | undefined;

  const cgroupBefore = await readCgroupCpuStat();

  for (let i = 0; i < amount; i++) {
    r.perf?.startRun();

    res = await r.execute(g, { randomSeed: true });

    r.perf?.stopRun();

    const { totalVertices, totalFaces } = countGeometry(res!);

    r.perf?.addToLastRun('total-vertices', totalVertices);
    r.perf?.addToLastRun('total-faces', totalFaces);
  }

  const cgroupAfter = await readCgroupCpuStat();

  clearInterval(sampler);

  log.log('finished');

  return {
    data: r.perf.get(),
    metadata: {
      timestamp: new Date().toISOString(),

      machine: getMachineInfo(),

      process: {
        pid: process.pid,
        uptime: process.uptime(),

        memoryUsage: process.memoryUsage()
      },

      system: {
        averages: {
          cpuUsagePercent: average(
            systemSamples.map(s => s.cpuUsagePercent)
          ),

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

        samples: systemSamples,

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
    log.log('executing ' + key);

    const perfData = await run(templates[key], 100);

    await writeFile(
      resolve(outPath, key + '.json'),
      JSON.stringify(perfData, null, 2)
    );

    await new Promise(res => setTimeout(res, 200));
  }
}

main();
