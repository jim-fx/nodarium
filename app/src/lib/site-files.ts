// The files the site serves for agents and tools next to the app, prerendered by the routes
// /nodes/index.*, /templates/*, /graph.schema.json and /docs/graphs.md
import { TEMPLATE_PREVIEW_DIR } from '#lib/project-manager/template-previews.ts';
import { formatNodeList, listNodes, nodeSummary } from '@nodarium/runtime/node';
import { templates } from '@nodarium/runtime/templates';
import { existsSync } from 'node:fs';
import { resolve } from 'node:path';

const REPOSITORY = 'https://git.max-richter.dev/max/nodarium/src/branch/main';

/** The nodes of the site, the same list as `nodarium nodes` */
export async function nodeCatalog() {
  const nodes = await listNodes(resolve('static/nodes'));
  return {
    text: formatNodeList(nodes) + '\n',
    json: {
      nodes: nodes.map((node) => ({
        ...nodeSummary(node),
        definition: `/nodes/${node.id}.json`
      }))
    }
  };
}

/** The templates of the project browser, without their graphs */
export async function templateIndex() {
  return Promise.all(templates.map(async (template) => {
    const graph = await template.load();
    const preview = `/${TEMPLATE_PREVIEW_DIR}/${template.id}.webp`;
    return {
      id: template.id,
      name: template.name,
      category: template.category,
      unit: template.unit,
      ...(template.species && { species: template.species }),
      ...(template.notes && { notes: template.notes }),
      nodes: graph.nodes.length + (graph.groups ?? []).reduce((n, g) => n + g.nodes.length, 0),
      graph: `/templates/${template.id}.json`,
      open: `/?template=${template.id}`,
      ...(existsSync(resolve(`static${preview}`)) && { preview })
    };
  }));
}

/** A doc of the repository on the site, its links to other docs point into the repository */
export function siteDoc(markdown: string) {
  return markdown.replace(/\]\(\.\/([\w.-]+\.md)\)/g, `](${REPOSITORY}/docs/$1)`);
}
