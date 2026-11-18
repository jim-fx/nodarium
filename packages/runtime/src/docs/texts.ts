import { readFileSync } from 'node:fs';

/** The files of `docs/` in the repository that the package ships, the build puts their text here */
export const DOC_FILES = [
  'GRAPHS.md',
  'PLANT_TEMPLATES.md',
  'CORE.md',
  'PLANTARIUM.md',
  'MYZEL.md'
] as const;

export type DocFile = (typeof DOC_FILES)[number];

export const DOC_TEXTS = Object.fromEntries(
  DOC_FILES.map((file) => [
    file,
    readFileSync(new URL(`../../../../docs/${file}`, import.meta.url), 'utf8')
  ])
) as Record<DocFile, string>;
