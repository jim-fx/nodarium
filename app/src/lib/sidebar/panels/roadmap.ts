export type RoadmapKind = 'feat' | 'concept';

export type RoadmapItem = {
  kind: RoadmapKind;
  number: number;
  title: string;
  html_url: string;
  created_at: string;
  updated_at: string;
  labels: string[];
  milestone?: string;
  summary?: string;
};

export type GiteaIssue = {
  number: number;
  title: string;
  html_url: string;
  created_at: string;
  updated_at: string;
  body?: string;
  labels?: { name: string }[] | null;
  milestone?: { title: string } | null;
  pull_request?: unknown;
};

const PREFIXES: [RegExp, RoadmapKind][] = [
  [/^feat(\([^)]*\))?:\s*/i, 'feat'],
  [/^(concept|konzept):\s*/i, 'concept']
];

const SUMMARY_LENGTH = 240;

function isProse(paragraph: string) {
  if (!paragraph) return false;
  if (/^(#|\||```|>|[-*] |\d+\. )/.test(paragraph)) return false;
  // source notes like _Quelle: Voice Note_ are not part of the description
  return !/^_[^_]+_$/.test(paragraph);
}

export function summarize(body?: string) {
  if (!body) return;
  const paragraph = body
    .replace(/^#.*$/gm, '')
    .split(/\n\s*\n/)
    .map((p) => p.trim())
    .find(isProse);
  if (!paragraph) return;
  const text = paragraph.replace(/\s+/g, ' ');
  return text.length > SUMMARY_LENGTH ? `${text.slice(0, SUMMARY_LENGTH - 1).trimEnd()}…` : text;
}

export function toRoadmapItem(issue: GiteaIssue): RoadmapItem | undefined {
  if (issue.pull_request) return;
  for (const [prefix, kind] of PREFIXES) {
    if (!prefix.test(issue.title)) continue;
    return {
      kind,
      number: issue.number,
      title: issue.title.replace(prefix, ''),
      html_url: issue.html_url,
      created_at: issue.created_at,
      updated_at: issue.updated_at,
      labels: issue.labels?.map((l) => l.name) ?? [],
      milestone: issue.milestone?.title,
      summary: summarize(issue.body)
    };
  }
}

export function sortRoadmap(items: RoadmapItem[]) {
  return items.toSorted((a, b) => {
    if (a.milestone !== b.milestone) {
      if (!a.milestone) return 1;
      if (!b.milestone) return -1;
      return a.milestone.localeCompare(b.milestone);
    }
    return b.updated_at.localeCompare(a.updated_at);
  });
}

export function groupByMilestone(items: RoadmapItem[]) {
  const groups = new Map<string | undefined, RoadmapItem[]>();
  for (const item of sortRoadmap(items)) {
    groups.set(item.milestone, [...(groups.get(item.milestone) ?? []), item]);
  }
  return [...groups].map(([milestone, items]) => ({ milestone, items }));
}
