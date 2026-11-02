// Writes the open feat: and concept issues to a json file for the roadmap tab of the changelog.
// A failed request writes an empty list, so the build never breaks on it.
//
// Usage: tsx scripts/fetch-roadmap.ts <output.json>
import { writeFile } from 'node:fs/promises';

import {
  type GiteaIssue,
  type RoadmapItem,
  toRoadmapItem
} from '../src/lib/sidebar/panels/roadmap.ts';

const API = process.env.ROADMAP_API ?? 'https://git.max-richter.dev/api/v1/repos/max/nodarium';
const PAGE_SIZE = 50;

const output = process.argv[2];
if (!output) {
  console.error('Usage: tsx scripts/fetch-roadmap.ts <output.json>');
  process.exit(1);
}

async function fetchOpenIssues() {
  const issues: GiteaIssue[] = [];
  for (let page = 1;; page++) {
    const res = await fetch(
      `${API}/issues?state=open&type=issues&limit=${PAGE_SIZE}&page=${page}`,
      {
        signal: AbortSignal.timeout(10_000)
      }
    );
    if (!res.ok) throw new Error(`${res.status} ${res.statusText}`);
    const batch: GiteaIssue[] = await res.json();
    issues.push(...batch);
    if (batch.length < PAGE_SIZE) return issues;
  }
}

let items: RoadmapItem[] = [];
try {
  items = (await fetchOpenIssues()).map(toRoadmapItem).filter((i) => i !== undefined);
  console.log(`roadmap: ${items.length} issues`);
} catch (error) {
  console.warn('roadmap: failed to fetch the issues, writing an empty roadmap', error);
}

await writeFile(output, JSON.stringify(items, null, 2) + '\n');
