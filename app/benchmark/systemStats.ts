import { readFile } from 'node:fs/promises';
import { cpus, totalmem } from 'node:os';

export type CpuSnapshot = {
  idle: number;
  total: number;
  steal: number;
};

export type SystemSample = {
  timestamp: number;
  cpuUsagePercent: number;
  cpuStealPercent: number;
  load1: number;
  load5: number;
  load15: number;
  freeMemory: number;
  totalMemory: number;
};

export async function readCpuSnapshot(): Promise<CpuSnapshot> {
  const stat = await readFile('/proc/stat', 'utf8');
  const line = stat.split('\n')[0];

  const parts: number[] = line
    .trim()
    .split(/\s+/)
    .slice(1)
    .map((v: unknown) => Number(v));

  const idle = parts[3];
  const iowait = parts[4];
  const steal = parts[7];

  return {
    idle: idle + iowait,
    total: parts.reduce((a, b) => a + b, 0),
    steal: steal ?? 0
  };
}

export async function measureCpuUsage(
  previous: CpuSnapshot
): Promise<{
  snapshot: CpuSnapshot;
  usagePercent: number;
  stealPercent: number;
}> {
  const current = await readCpuSnapshot();

  const idle = current.idle - previous.idle;
  const total = current.total - previous.total;
  const steal = current.steal - previous.steal;

  return {
    snapshot: current,
    usagePercent: total === 0 ? 0 : 100 * (1 - idle / total),
    stealPercent: total === 0 ? 0 : 100 * (steal / total)
  };
}

export async function readCgroupCpuStat() {
  const possiblePaths = [
    '/sys/fs/cgroup/cpu.stat',
    '/sys/fs/cgroup/cpu/cpu.stat'
  ];

  for (const path of possiblePaths) {
    try {
      const txt: string = await readFile(path, 'utf8');

      return Object.fromEntries(
        txt
          .trim()
          .split('\n')
          .map(line => {
            const [k, v] = line.trim().split(/\s+/);
            return [k, Number(v)];
          })
      );
    } catch {
      // continue
    }
  }

  return null;
}

export async function readProcMemInfo() {
  try {
    const txt = await readFile('/proc/meminfo', 'utf8');

    const result: Record<string, number> = {};

    for (const line of txt.split('\n')) {
      const match = line.match(/^(\w+):\s+(\d+)/);

      if (!match) continue;

      result[match[1]] = Number(match[2]);
    }

    return result;
  } catch {
    return null;
  }
}

export function getMachineInfo() {
  const cpuInfo = cpus();

  return {
    platform: process.platform,
    arch: process.arch,
    nodeVersion: process.version,

    cpuModel: cpuInfo[0]?.model ?? 'unknown',
    cpuCount: cpuInfo.length,

    totalMemory: totalmem(),

    ci: {
      githubActions: process.env.GITHUB_ACTIONS ?? false,
      runnerName: process.env.RUNNER_NAME ?? null,
      runnerOs: process.env.RUNNER_OS ?? null,
      runnerArch: process.env.RUNNER_ARCH ?? null
    }
  };
}
