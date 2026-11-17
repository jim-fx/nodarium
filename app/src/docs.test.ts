// AGENTS.md and the docs name paths and pnpm scripts. They went stale before because nothing
// noticed when a file moved or a script was renamed, so this test reads them and checks both.
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

const ROOT = resolve(import.meta.dirname, '../..');

const DOCS = [
  'AGENTS.md',
  'README.md',
  'packages/runtime/README.md',
  ...readdirSync(join(ROOT, 'docs')).filter((f) => f.endsWith('.md')).map((f) => `docs/${f}`)
];

// built by `pnpm build:nodes` or cargo, not in a fresh checkout
const GENERATED = ['app/static/nodes', 'target', 'node_modules'];

// pnpm's own commands and the binaries the docs call through it
const PNPM_COMMANDS = new Set(['i', 'install', 'add', 'exec', 'dlx', 'dprint']);

const TOP_LEVEL = new Set(
  readdirSync(ROOT).filter((entry) => !GENERATED.includes(entry) && entry !== '.git')
);

function packageScripts(dir: string): Set<string> {
  const file = join(ROOT, dir, 'package.json');
  if (!existsSync(file)) return new Set();
  return new Set(Object.keys(JSON.parse(readFileSync(file, 'utf8')).scripts ?? {}));
}

const PACKAGES = ['.', 'app', ...readdirSync(join(ROOT, 'packages')).map((p) => `packages/${p}`)];
const ALL_SCRIPTS = new Set(PACKAGES.flatMap((dir) => [...packageScripts(dir)]));

/** The inline code spans and the lines of the fenced code blocks of a markdown file */
function codeOf(markdown: string): { spans: string[]; lines: string[] } {
  const lines: string[] = [];
  const prose = markdown.replace(/^```[^\n]*\n([\s\S]*?)^```/gm, (_, block: string) => {
    lines.push(...block.split('\n'));
    return '';
  });
  const spans = [...prose.matchAll(/`([^`\n]+)`/g)].map((m) => m[1]);
  return { spans, lines };
}

/** A span that names a file or directory of the repository, relative to its root */
function repoPath(span: string): string | undefined {
  const path = span.replace(/^\.\//, '').replace(/\/$/, '');
  if (!/^[\w.@-]+(\/[\w.@-]+)*$/.test(path)) return undefined;
  if (!TOP_LEVEL.has(path.split('/')[0])) return undefined;
  if (GENERATED.some((g) => path === g || path.startsWith(`${g}/`))) return undefined;
  return path;
}

/** The scripts a line runs with pnpm, with the directory whose package.json must have them */
function pnpmScripts(line: string): { dir?: string; script: string }[] {
  const found: { dir?: string; script: string }[] = [];
  for (const match of line.matchAll(/(?:^|[\s(`&;|])pnpm((?:\s+[^\s&;|`)]+)+)/g)) {
    const words = match[1].trim().split(/\s+/);
    let dir: string | undefined;
    let i = 0;
    while (i < words.length && (words[i].startsWith('-') || words[i] === 'run')) {
      if (words[i] === '-C' || words[i] === '--dir') dir = words[++i];
      else if (words[i] === '--filter' || words[i] === '-F') i++;
      i++;
    }
    const script = words[i];
    if (script && !PNPM_COMMANDS.has(script)) found.push({ dir, script });
  }
  return found;
}

describe('the docs', () => {
  for (const doc of DOCS) {
    describe(doc, () => {
      const { spans, lines } = codeOf(readFileSync(join(ROOT, doc), 'utf8'));

      it('names only paths that exist', () => {
        const missing = spans
          .map(repoPath)
          .filter((path): path is string => !!path && !existsSync(join(ROOT, path)));
        expect(missing).toEqual([]);
      });

      it('runs only pnpm scripts that exist', () => {
        const unknown = [...spans, ...lines]
          .flatMap(pnpmScripts)
          .filter(({ dir, script }) =>
            dir === undefined ? !ALL_SCRIPTS.has(script) : !packageScripts(dir).has(script)
          )
          .map(({ dir, script }) => (dir ? `pnpm -C ${dir} ${script}` : `pnpm ${script}`));
        expect(unknown).toEqual([]);
      });
    });
  }
});
