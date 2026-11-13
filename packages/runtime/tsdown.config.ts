import { readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { defineConfig } from 'tsdown';

const templateIds = readdirSync('templates')
  .filter((file) => file.endsWith('.json'))
  .map((file) => file.slice(0, -5));

const templateDeclaration =
  "import type { Graph } from '../index.js';\ndeclare const graph: Graph;\nexport default graph;\n";

export default defineConfig({
  entry: {
    index: 'src/index.ts',
    node: 'src/node.ts',
    'templates/index': 'templates/index.ts'
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
      // the templates are written by the build:done hook, the dts plugin needs gigabytes for
      // the declarations it would generate from them
      name: 'nodarium-templates',
      resolveId: {
        filter: { id: /^\.\/[^\/]+\.json$/ },
        handler(source, importer) {
          if (!importer || !/[\\/]templates[\\/]index\.ts$/.test(importer)) return;
          return { id: source.replace(/\.json$/, '.js'), external: true };
        }
      }
    }
  ],
  hooks: {
    // every template is a file of its own, `@nodarium/runtime/templates/<id>`, so a bundler
    // splits them into chunks of their own. JSON.parse reads a large object faster than a
    // literal, and a template is typed as a Graph instead of spelling out every node
    'build:done'() {
      for (const id of templateIds) {
        const json = JSON.stringify(JSON.parse(readFileSync(`templates/${id}.json`, 'utf8')));
        writeFileSync(
          `dist/templates/${id}.js`,
          `export default JSON.parse(${JSON.stringify(json)});\n`
        );
        writeFileSync(`dist/templates/${id}.d.ts`, templateDeclaration);
      }
    }
  },
  dts: { tsconfig: 'tsconfig.build.json', tsc: { eager: true } },
  clean: true
});
