# Nodarium App

## Template previews

The project browser shows the templates of `packages/runtime/templates` with previews from `static/template-previews`, rendered ahead of time, so opening it builds no template. `static/template-previews/manifest.json` keeps the hash of the template graph each image was rendered from (the nodes, their props, the edges and the settings, not the positions). The hashes are compared when the app is built (`scripts/template-previews-plugin.ts`): a template whose image is missing or was rendered from an older version of it gets its preview built live in the browser, until the images are rendered again.

After changing or adding a template, render the previews again and commit the images with it:

```sh
pnpm build:nodes                     # in the repository root, the renderer needs static/nodes
pnpm render-template-previews        # in app/, renders the missing and stale previews
pnpm render-template-previews --all  # after a node changed the look of the templates
pnpm render-template-previews fern   # only these templates
```

The script starts the dev server and renders each template with the thumbnail worker of the app in a headless Chromium. It uses the Chromium of Playwright, `--chromium <path>` uses another one (for example `/usr/bin/chromium`). A change to a node doesn't change the hash of a template, so after one that changes how the templates look, render all of them with `--all`.
