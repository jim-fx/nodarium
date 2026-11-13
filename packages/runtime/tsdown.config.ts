import { defineConfig } from 'tsdown';

export default defineConfig({
  entry: ['src/index.ts', 'src/node.ts'],
  format: 'esm',
  platform: 'neutral',
  target: 'es2022',
  // types and utils are workspace packages that are not published, their code and types go
  // into the bundle, zod stays a dependency
  deps: { alwaysBundle: ['@nodarium/types', '@nodarium/utils'], neverBundle: [/^node:/] },
  // utils only has a main field, which the neutral platform ignores by default
  inputOptions: { resolve: { mainFields: ['module', 'main'] } },
  dts: { tsconfig: 'tsconfig.build.json', tsc: { eager: true } },
  clean: true
});
