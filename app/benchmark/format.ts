import { type Change, compareMedians, type Summary } from './stats.ts';

/** Two significant digits at least: 662, 3.8, 0.15, 0.042 */
export function ms(value: number) {
  if (!Number.isFinite(value)) return '-';
  if (value === 0) return '0';
  const digits = value >= 100 ? 0 : value >= 1 ? 1 : value >= 0.1 ? 2 : 3;
  return value.toFixed(digits);
}

export function count(value: number) {
  if (value >= 1e6) return (value / 1e6).toFixed(1) + ' M';
  if (value >= 1e3) return (value / 1e3).toFixed(1) + ' k';
  return String(Math.round(value));
}

export function bytes(value: number) {
  if (value >= 1 << 20) return (value / (1 << 20)).toFixed(1) + ' MB';
  if (value >= 1 << 10) return (value / (1 << 10)).toFixed(1) + ' kB';
  return Math.round(value) + ' B';
}

/** Whole percents, one decimal below 1 % */
export function percent(value: number, signed = false) {
  const text = Math.abs(value) < 0.01 && value !== 0
    ? `${(value * 100).toFixed(1)} %`
    : `${Math.round(value * 100)} %`;
  return signed && value >= 0 ? '+' + text : text;
}

/** Rows of cells as columns, padded to the widest cell, numbers right-aligned */
export function table(rows: string[][], right: number[] = []) {
  const widths = rows[0].map((_, i) => Math.max(...rows.map(row => (row[i] ?? '').length)));
  return rows
    .map(row =>
      row
        .map((cell, i) => {
          if (i === row.length - 1) return cell;
          return right.includes(i) ? cell.padStart(widths[i]) : cell.padEnd(widths[i]);
        })
        .join('   ')
        .trimEnd()
    )
    .join('\n');
}

const label = (summary: Summary) => summary.name + (summary.cached ? '*' : '');
const shortNode = (node: string) => node.split('/').pop()!;

export function summaryTable(summaries: Summary[], topNodes = 3) {
  const rows = summaries.map(s => [
    label(s),
    ms(s.median) + ' ms',
    ms(s.p95),
    percent(s.spread),
    count(s.vertices),
    s.nodes.slice(0, topNodes).map(n => `${shortNode(n.node)} ${ms(n.ms)}`).join(', ')
  ]);
  const text = table([['template', 'median', 'p95', '±', 'vertices', 'top nodes'], ...rows], [
    1,
    2,
    3,
    4
  ]);
  return summaries.some(s => s.cached) ? text + '\n(* cached)' : text;
}

export type Comparison = {
  name: string;
  cached: boolean;
  total?: Change;
  p95?: { before: number; after: number };
  spread?: { before: number; after: number };
  nodes: { node: string; before: number; after: number }[];
};

const key = (s: Summary) => s.name + (s.cached ? '-cached' : '');

export function compareSummaries(before: Summary[], after: Summary[], topNodes = 3): Comparison[] {
  const old = new Map(before.map(s => [key(s), s]));
  return after.map(s => {
    const b = old.get(key(s));
    if (!b) return { name: s.name, cached: s.cached, nodes: [] };
    const previous = new Map(b.nodes.map(n => [n.node, n.ms]));
    return {
      name: s.name,
      cached: s.cached,
      total: compareMedians(b, s),
      p95: { before: b.p95, after: s.p95 },
      spread: { before: b.spread, after: s.spread },
      nodes: s.nodes.slice(0, topNodes).map(n => ({
        node: n.node,
        before: previous.get(n.node) ?? 0,
        after: n.ms
      }))
    };
  });
}

export function comparisonTable(comparisons: Comparison[]) {
  const rows = comparisons.map(c => {
    if (!c.total) {
      return [c.name + (c.cached ? '*' : ''), '-', '', '', '', '', 'not in the compared run'];
    }
    return [
      c.name + (c.cached ? '*' : ''),
      `${ms(c.total.before)} → ${ms(c.total.after)} ms`,
      percent(c.total.change, true),
      c.total.verdict,
      `${ms(c.p95!.before)} → ${ms(c.p95!.after)}`,
      `${percent(c.spread!.before)} / ${percent(c.spread!.after)}`,
      c.nodes.map(n => `${shortNode(n.node)} ${ms(n.before)} → ${ms(n.after)}`).join(', ')
    ];
  });
  const header = ['template', 'median', 'change', '', 'p95', '±', 'top nodes'];
  const text = table([header, ...rows], [1, 2, 4]);
  return comparisons.some(c => c.cached) ? text + '\n(* cached)' : text;
}
