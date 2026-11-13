import { readdirSync, writeFileSync } from 'node:fs';
import { defineConfig } from 'tsdown';

const templateIds = readdirSync('templates')
  .filter((file) => file.endsWith('.json'))
  .map((file) => file.slice(0, -5));

export default defineConfig({
  entry: {
    index: 'src/index.ts',
    node: 'src/node.ts',
    'templates/index': 'templates/index.ts',
    // every template is an entry of its own, `@nodarium/runtime/templates/<id>`
    ...Object.fromEntries(templateIds.map((id) => [`templates/${id}`, `templates/${id}.json`]))
  },
  format: 'esm',
  platform: 'neutral',
  target: 'es2022',
  // types and utils are workspace packages that are not published, their code and types go
  // into the bundle, zod stays a dependency
  deps: { alwaysBundle: ['@nodarium/types', '@nodarium/utils'], neverBundle: [/^node:/] },
  // utils only has a main field, which the neutral platform ignores by default
  inputOptions: { resolve: { mainFields: ['module', 'main'] } },
  plugins: [
    {
      // a template is one default export, JSON.parse reads a large object faster than a literal
      name: 'nodarium-templates',
      transform: {
        filter: { id: /[\\/]templates[\\/][^\\/]+\.json$/ },
        handler(code) {
          const json = JSON.stringify(JSON.parse(code));
          return { code: `export default JSON.parse(${JSON.stringify(json)});`, moduleType: 'js' };
        }
      }
    }
  ],
  hooks: {
    // the declarations generated from the json spell out every node, a template is a Graph
    'build:done'() {
      for (const id of templateIds) {
        writeFileSync(
          `dist/templates/${id}.d.ts`,
          "import type { Graph } from '../index.js';\ndeclare const graph: Graph;\nexport default graph;\n"
        );
      }
    }
  },
  dts: { tsconfig: 'tsconfig.build.json', tsc: { eager: true } },
  clean: true
});
