import { describe, expect, it } from 'vitest';
import {
  type GiteaIssue,
  groupByMilestone,
  type RoadmapItem,
  summarize,
  toRoadmapItem
} from './roadmap';

function issue(title: string, extra: Partial<GiteaIssue> = {}): GiteaIssue {
  return {
    number: 1,
    title,
    html_url: 'https://git.max-richter.dev/max/nodarium/issues/1',
    created_at: '2026-11-01T00:00:00Z',
    updated_at: '2026-11-01T00:00:00Z',
    ...extra
  };
}

function item(number: number, updated_at: string, milestone?: string): RoadmapItem {
  return {
    kind: 'feat',
    number,
    title: `item ${number}`,
    html_url: '',
    created_at: updated_at,
    updated_at,
    labels: [],
    milestone
  };
}

describe('toRoadmapItem', () => {
  it('keeps feat: issues and strips the prefix', () => {
    expect(toRoadmapItem(issue('feat: umbel node'))).toMatchObject({
      kind: 'feat',
      title: 'umbel node'
    });
    expect(toRoadmapItem(issue('feat(ui): roadmap tab'))?.title).toBe('roadmap tab');
  });

  it('keeps concept issues in english and german', () => {
    expect(toRoadmapItem(issue('concept: energy-flow'))).toMatchObject({
      kind: 'concept',
      title: 'energy-flow'
    });
    expect(toRoadmapItem(issue('Konzept: Myzel'))?.kind).toBe('concept');
  });

  it('drops other issues and pull requests', () => {
    expect(toRoadmapItem(issue('fix: broken thing'))).toBeUndefined();
    expect(toRoadmapItem(issue('chore: rename'))).toBeUndefined();
    expect(toRoadmapItem(issue('feature without colon'))).toBeUndefined();
    expect(toRoadmapItem(issue('feat: a pr', { pull_request: {} }))).toBeUndefined();
  });

  it('reads labels and the milestone', () => {
    const result = toRoadmapItem(
      issue('feat: x', { labels: [{ name: 'node' }], milestone: { title: 'v0.1' } })
    );
    expect(result?.labels).toEqual(['node']);
    expect(result?.milestone).toBe('v0.1');
  });
});

describe('summarize', () => {
  it('takes the first paragraph that is not a heading', () => {
    expect(summarize('## Goal\n\nGrow   plants\nfrom points.\n\nMore text')).toBe(
      'Grow plants from points.'
    );
  });

  it('reads text written right below a heading', () => {
    expect(summarize('## Idee\nPoints along paths.\n\n## Verhalten\n- a')).toBe(
      'Points along paths.'
    );
  });

  it('skips source notes, quotes and lists', () => {
    expect(summarize('_Quelle: Voice Note 2026-10-31_\n\n> quote\n\n- a\n- b\n\nThe idea.')).toBe(
      'The idea.'
    );
  });

  it('shortens long paragraphs', () => {
    const summary = summarize('a'.repeat(500));
    expect(summary?.length).toBe(240);
    expect(summary?.endsWith('…')).toBe(true);
  });

  it('returns nothing for an empty body', () => {
    expect(summarize('')).toBeUndefined();
    expect(summarize('## Only a heading')).toBeUndefined();
  });
});

describe('groupByMilestone', () => {
  it('puts milestones first and the most recently updated issues on top', () => {
    const groups = groupByMilestone([
      item(1, '2026-10-01'),
      item(2, '2026-11-01'),
      item(3, '2026-10-01', 'v1'),
      item(4, '2026-11-01', 'v1')
    ]);
    expect(groups.map((g) => [g.milestone, g.items.map((i) => i.number)])).toEqual([
      ['v1', [4, 3]],
      [undefined, [2, 1]]
    ]);
  });
});
