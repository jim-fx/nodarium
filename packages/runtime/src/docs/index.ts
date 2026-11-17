import { type Template, templates } from '@nodarium/runtime/templates';
import { DOC_TEXTS, type DocFile } from './texts';

export const REPOSITORY_DOCS = 'https://git.max-richter.dev/max/nodarium/src/branch/main/docs';

export type DocTopic = {
  /** what `nodarium docs <topic>` and the tool `get_docs` take */
  topic: string;
  file: DocFile;
  about: string;
};

/** The docs of the repository that come with the package, in the order to read them */
export const DOC_TOPICS: DocTopic[] = [
  { topic: 'graphs', file: 'GRAPHS.md', about: 'how to write a graph by hand, start here' },
  {
    topic: 'plant-templates',
    file: 'PLANT_TEMPLATES.md',
    about: 'what the plant nodes can express, and how every plant template models its species'
  },
  {
    topic: 'plantarium',
    file: 'PLANTARIUM.md',
    about: 'the data of the plant nodes: paths, points, leaves, geometry, instances, colors'
  },
  { topic: 'myzel', file: 'MYZEL.md', about: 'the mushroom nodes, their groups and workflow' }
];

/** The templates as `id: name (category, unit)`, the species and how each is built below */
export function templateList(list: Template[] = templates) {
  return list.map((t) => {
    const line = `${t.id}: ${t.name} (${t.category}, ${t.unit})`;
    const about = [t.species, t.notes].filter(Boolean).join(': ');
    return about ? `${line}\n  ${about}` : line;
  }).join('\n');
}

/** A doc by its topic or its file name, `plantarium` or `PLANTARIUM.md` */
export function findDoc(name: string) {
  const wanted = name.trim().toLowerCase();
  return DOC_TOPICS.find((d) => d.topic === wanted || d.file.toLowerCase() === wanted);
}

/**
 * A doc as the package gives it. A link to another doc points into the repository, and to
 * `nodarium docs <topic>` as well where the package has that doc.
 */
export function docText(topic: string): string | undefined {
  if (topic.trim().toLowerCase() === 'templates') return templateList();
  const doc = findDoc(topic);
  if (!doc) return undefined;
  return DOC_TEXTS[doc.file].replace(
    /\]\(\.\/([\w.-]+\.md)((?:#[\w-]*)?)\)/g,
    (_, file, anchor) => {
      const shipped = DOC_TOPICS.find((d) => d.file === file);
      const link = `](${REPOSITORY_DOCS}/${file}${anchor})`;
      return shipped ? `${link} (\`nodarium docs ${shipped.topic}\`)` : link;
    }
  );
}

/** What `nodarium docs` prints without a topic */
export function docList() {
  const rows = [
    ...DOC_TOPICS.map((d) => [d.topic, d.file, d.about]),
    ['templates', '', 'every template of the app, with the species and how it is built']
  ];
  const topics = Math.max(...rows.map((r) => r[0].length));
  const files = Math.max(...rows.map((r) => r[1].length));
  return rows.map(([topic, file, about]) =>
    `${topic.padEnd(topics)}  ${file.padEnd(files)}  ${about}`
  )
    .join('\n');
}
