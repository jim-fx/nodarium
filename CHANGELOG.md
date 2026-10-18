# v0.0.7 (2026-10-18)

- Add plant templates modelled on real species, including a new Salvia officinalis (sage) template.
- Add a title bar with menus and a project browser window.
- Render project thumbnails in a worker and import/export project files.
- Add a /garden route that plants the recent projects.
- Lay out the nodes of a graph in columns (Shift+F).
- Add toasts, a zoom indicator and a confirm dialog when deleting a project.
- Render templates and projects to screenshots headlessly (`pnpm render-graph`).
- Add a language agnostic node ABI v1.

Plant templates describe real species, from the agave to the weeping willow, and are documented in `PLANT_TEMPLATES.md`.

- Agave, bamboo, fern, ornamental grass, lavender, palm, sapling, weeping willow and sage
- Sage with opposite decussate leaves, axillary leaf tufts and whorls of two-lipped flowers
- Fern and lavender grow from a crown at the golden angle, lavender from a low woody framework
- Bamboo grows as a grove of almost straight culms, the grass gets real leaf blades
- The birch sapling gets a slender trunk, hanging twigs and small rhombic leaves
- Shaped with the new alpha, curve and angle inputs
- Test graphs (Plant, Simple, Lotta Faces) are no longer listed in the project browser

New nodes and inputs make it possible to vary branches and leaves along their parent.

- Alpha node for where a branch or instance sits on its parent path
- Curve node that shapes a 0-1 value
- Stem and branch thickness can follow alpha along the path
- Instances can tilt along their path with an angle input
- Phyllotactic spiral and upward angle for branches
- Noise can preserve segment lengths during displacement

- Keep the values of a graph when its number inputs are rendered
- Extrude paths with segments of zero length without NaN positions
- Only center the camera on a finite bounding box, a NaN geometry turned the view black
- Show every leaf and flower after switching projects, some stayed hidden until a reload
- Fix undo inside groups, multi-output wiring and input removal
- Nest branches below their parent and orient instances sideways
- Give vertical and collapsed path segments a valid frame
- Instantiate a node module only when it is executed
- Load each node once and list nodes by their .wasm files
- Reset modules of node types that left the graph
- Keep rendering edges of a node whose type failed to load
- Fix pasting nodes and the graph background on theme switch

- Evaluate float expressions lazily on a per-run context
- Use the new button component everywhere

- Make tracked benchmark results reliable and run them in CI
- Cover the runtime cache, module resets and the wasm wrapper with tests
- Add UX guidelines
- Format the layout of all graph templates

---

- [a48c4ae](https://git.max-richter.dev/max/nodarium/commit/a48c4aeffea0a1bce87258752a243469df362728) test(e2e): create the project from the Default Plant template
- [d3ba40e](https://git.max-richter.dev/max/nodarium/commit/d3ba40eec3bb21b4d1c9cc60e7818d1d428cd50f) feat(templates): make the plant templates more realistic
- [f56ea15](https://git.max-richter.dev/max/nodarium/commit/f56ea15379c0da8efe06db5ace814227c3e02a6c) feat(templates): add a Salvia officinalis (sage) template
- [07249c2](https://git.max-richter.dev/max/nodarium/commit/07249c201b96ecf707dd78598c9510e44b475afb) fix(viewer): show every instanced geometry after switching graphs
- [9e70480](https://git.max-richter.dev/max/nodarium/commit/9e704806e2a60ce625243bf84a2d17b77027309d) fix(viewer): only center the camera on a finite bounding box
- [510571b](https://git.max-richter.dev/max/nodarium/commit/510571b5bd64bd3004b030d50abc0a28f56f39de) fix(utils): extrude paths with segments of zero length
- [4dcb424](https://git.max-richter.dev/max/nodarium/commit/4dcb42442810f52e1002acceb8e18e251ffd2a21) fix(ui): keep the value of a number input when it is rendered
- [3205c9d](https://git.max-richter.dev/max/nodarium/commit/3205c9dec28abed3bf8b6e316b4df808c6e3b4e8) chore(templates): hide the Plant, Simple and Lotta Faces templates in the project browser
- [8406158](https://git.max-richter.dev/max/nodarium/commit/8406158a375f9f74efcefa46a9d04fb06ae642aa) Merge pull request 'Feat/plant templates alpha' (#53) from feat/plant-templates-alpha into main
- [5aa76cb](https://git.max-richter.dev/max/nodarium/commit/5aa76cb069ec3b1c6ee4358d673da13bad2cac02) chore(templates): format the layout of all graph templates
- [39f4c55](https://git.max-richter.dev/max/nodarium/commit/39f4c5577ad868dedd78be2a24f7a5c0b8c35512) feat(graph): lay out the nodes of a graph in columns
- [580244d](https://git.max-richter.dev/max/nodarium/commit/580244d345d256a67c4e77c85bf5068ce2c48bab) feat(templates): shape the plant templates with alpha, curve and angle
- [6e66003](https://git.max-richter.dev/max/nodarium/commit/6e660038bb2bbce8020421e96015376e35e25bd1) feat(nodes): tilt instances along their path with an angle input
- [9e4066e](https://git.max-richter.dev/max/nodarium/commit/9e4066e99dfc9d49d5384f6b431f6dd158405f8d) Merge pull request 'Feat/alpha node' (#51) from feat/alpha-node into main
- [ced2d1e](https://git.max-richter.dev/max/nodarium/commit/ced2d1e452cbc47236334fdb87acb7ce44363355) fix(performance): draw graphs with a single point or a flat line
- [6e2a677](https://git.max-richter.dev/max/nodarium/commit/6e2a67768bf1b226526a01d1ec25335f420a06fb) fix(thumbnails): send plain graphs to the thumbnail worker
- [393a1dd](https://git.max-richter.dev/max/nodarium/commit/393a1dd09414e0ed210c8e5c40144490f87e2f45) fix(graph): keep rendering edges of a node whose type failed to load
- [21e2b0c](https://git.max-richter.dev/max/nodarium/commit/21e2b0c883911af684996ffd241e3851bc21754a) fix(utils): instantiate a node module only when it is executed
- [89c70c5](https://git.max-richter.dev/max/nodarium/commit/89c70c5c811aae258d8fcb4887bebbacd4c5d41d) feat(nodes): let stem and branch thickness follow alpha along the path
- [f84e1a0](https://git.max-richter.dev/max/nodarium/commit/f84e1a038516a265877db340b8fb3286cbb8916f) feat(nodes): show only the curve and a value socket on the curve node
- [180047b](https://git.max-richter.dev/max/nodarium/commit/180047b46e9e109ff50f0598eac76a42f42e4eca) feat(nodes): add curve node that shapes a 0-1 value
- [a9b1667](https://git.max-richter.dev/max/nodarium/commit/a9b166737707f0f01253f1a8ef0a51adb65650dc) fix(macros): rebuild a node when its definition file changes
- [96cd148](https://git.max-richter.dev/max/nodarium/commit/96cd14834e62899fb262fbe320738e6217470a79) fix(utils): give vertical and collapsed path segments a valid frame
- [c4e290b](https://git.max-richter.dev/max/nodarium/commit/c4e290b6a6e7f35a7525c44f7d9bf27337dfedf1) feat(nodes): add alpha node for where a branch or instance sits on its parent
- [b092582](https://git.max-richter.dev/max/nodarium/commit/b0925824e05c417e0a78b755d7f9acea5a59f17f) refactor(utils): evaluate float expressions lazily on a per-run Ctx
- [a2cb937](https://git.max-richter.dev/max/nodarium/commit/a2cb93774dce63e70cb19978ff684b77ca054ad4) chore: remove the wasm route and format the plant templates doc
- [4a42513](https://git.max-richter.dev/max/nodarium/commit/4a425130c87ea099dc572b80fc5e8cb21d2d319d) feat(scripts): render templates and projects to screenshots headlessly
- [a15fe01](https://git.max-richter.dev/max/nodarium/commit/a15fe019b63d6f8b624bd14d5455430500c2de83) feat(garden): add /garden route that plants the recent projects
- [79a729a](https://git.max-richter.dev/max/nodarium/commit/79a729a39f9a9223a2ca2b73668727caded7fa34) feat(templates): add plant templates modelled on real species
- [b2feeda](https://git.max-richter.dev/max/nodarium/commit/b2feeda566bdacea3af5b634a87d67601671bd55) fix(registry): list nodes by their .wasm files and load each node once
- [ef90fea](https://git.max-richter.dev/max/nodarium/commit/ef90fea50dbb06c585b399cd8844ab13c4ec7e2a) fix(nodes): nest branches below their parent and orient instances sideways
- [2b6a2d5](https://git.max-richter.dev/max/nodarium/commit/2b6a2d52db87737da6517264d961dea93a478a8c) Merge pull request 'fix(groups): fix the remaining node group bugs' (#50) from fix/remaining-group-bugs into main
- [86100ee](https://git.max-richter.dev/max/nodarium/commit/86100ee043f9f8a2529bad76ae029d0c6fd78ee2) fix(groups): fix the remaining node group bugs
- [7d9b055](https://git.max-richter.dev/max/nodarium/commit/7d9b055f0d91ffa3fc46b7be1fad1f6659cde334) Merge pull request 'feat: project ui' (#47) from feat/project-ui into main
- [650a3c1](https://git.max-richter.dev/max/nodarium/commit/650a3c1ab2d9ed4acc93b36dc587d971e07f7f86) Merge branch 'main' into feat/project-ui
- [3ece245](https://git.max-richter.dev/max/nodarium/commit/3ece24588b04ab0bd2dac185af5c11e3837f8549) Merge pull request 'fix(groups): fix undo inside groups, multi-output wiring and input removal' (#49) from fix/group-bugs into main
- [b6f6d8c](https://git.max-richter.dev/max/nodarium/commit/b6f6d8c688c33791f79431792fce3c086be05d4f) test(e2e): create the project through the title bar's project browser
- [0c76d20](https://git.max-richter.dev/max/nodarium/commit/0c76d204f415fb192b6ff4990eeeb4f5f21b7658) Merge branch 'main' into feat/project-ui
- [08e98a3](https://git.max-richter.dev/max/nodarium/commit/08e98a352ad681c8f1f34b4ef2b6b8efdcadc3d9) Merge branch 'main' into fix/group-bugs
- [b06a390](https://git.max-richter.dev/max/nodarium/commit/b06a390f21c0893bc0559eb9888c18c711b8a847) Merge pull request 'Feat/wasm abi v1' (#45) from feat/wasm-abi-v1 into main
- [59fca8a](https://git.max-richter.dev/max/nodarium/commit/59fca8abc862084cc0895e56d7fd2d6eaa02b6cb) fix(groups): fix undo inside groups, multi-output wiring and input removal
- [92bd1ca](https://git.max-richter.dev/max/nodarium/commit/92bd1ca89e5ffb5c6f030ff2c6c9ca58887128c2) feat(ui): use sidebar panel icons for the sidebar toggle
- [9cc4534](https://git.max-richter.dev/max/nodarium/commit/9cc4534d7da5e6e4c20350b06587a3206d635293) fix(ui): keep project browser borders at full opacity and centre the template label
- [c0a5052](https://git.max-richter.dev/max/nodarium/commit/c0a5052683b44985a2d7cb3c8b78c6230db31d41) fix(ui): align the project browser import button with the footer
- [a023167](https://git.max-richter.dev/max/nodarium/commit/a0231679d336212d5788a1fa21e99a97efbfc3d5) feat(ui): add a title bar with menus and a project browser window
- [9aa324a](https://git.max-richter.dev/max/nodarium/commit/9aa324a6e97f4da17e22a312eef1b46d5da89165) feat: render project thumbnails in a worker and add project file import/export
- [07ac241](https://git.max-richter.dev/max/nodarium/commit/07ac241d118285d894a7421ee2c7689f0e20ab89) test(utils): cover the wasm wrapper with hand written test modules
- [1470484](https://git.max-richter.dev/max/nodarium/commit/147048470f9e7a42dd03e164e29cfe23a726aa84) test(runtime): cover the structural cache keys and module resets
- [8f89cdd](https://git.max-richter.dev/max/nodarium/commit/8f89cdd7c08eb3045faeea7efaaf2fe45b457f51) fix(runtime): reset modules of node types that left the graph
- [d18f4ee](https://git.max-richter.dev/max/nodarium/commit/d18f4eec1483ad3ade03aeea3975d253b7b6b965) fix(ui): keep the chosen select option of node settings, default gravity to chain
- [cb3b9c0](https://git.max-richter.dev/max/nodarium/commit/cb3b9c048bc8ce7c982f9da2c7cd96fdd64e1a4a) docs: keep the host node-agnostic, formats belong to node systems
- [2841c86](https://git.max-richter.dev/max/nodarium/commit/2841c8637272c7e7f9afb60ae21a70e377d98540) fix(benchmark): read the instance count from the right header field
- [a53e974](https://git.max-richter.dev/max/nodarium/commit/a53e9742688ea91f118142966ebef9fe3ff10d3d) fix(ci): push benchmark results through gitea's ssh port 22
- [a2e1473](https://git.max-richter.dev/max/nodarium/commit/a2e14732b78f32a13358dee2fa4b6812483b45d9) ci(benchmark): limit the benchmark container to 2GB without swap
- [f5b0890](https://git.max-richter.dev/max/nodarium/commit/f5b0890aed71c9733b965a404dcecc6a1c52113c) fix(benchmark): reuse wasm instances instead of creating them per run
- [bc978e7](https://git.max-richter.dev/max/nodarium/commit/bc978e7ca751f228ee14004681a868e4e21891fd) fix(ci): benchmark branches containing a slash, format benchmark
- [520aaeb](https://git.max-richter.dev/max/nodarium/commit/520aaeb004f3abe9ecad4541bf2bb9116753beee) feat(benchmark): make tracked results reliable
- [210c12e](https://git.max-richter.dev/max/nodarium/commit/210c12e9f6777696f8b22532c2dbbe1db4594ea6) feat(runtime): add language agnostic node ABI v1
- [4dd5f63](https://git.max-richter.dev/max/nodarium/commit/4dd5f633d4de97f2511c7d099e9977f3385de4c0) fix: make eslint/typescript happy
- [e6c368a](https://git.max-richter.dev/max/nodarium/commit/e6c368afaa484cdacbed4702dee5c67a9d917b1c) feat: update some nodes a bit
- [581daa1](https://git.max-richter.dev/max/nodarium/commit/581daa1be771550ef2e8b24a76cfb2b523e6737c) feat(noise): add preserveLength toggle
- [f652b71](https://git.max-richter.dev/max/nodarium/commit/f652b712df20dbe439265208c3dd362b948f5bb3) fix: remove redundant schema from union
- [68ae625](https://git.max-richter.dev/max/nodarium/commit/68ae62527fba1af4dc6994cdb9cc12684da2683a) feat(noise): preserve segment lengths during displacement
- [49746c6](https://git.max-richter.dev/max/nodarium/commit/49746c607927b319f049efe40e03881f61037109) feat(branch): phyllotactic spiral and upward angle
- [e5df19b](https://git.max-richter.dev/max/nodarium/commit/e5df19b6d88ca183616311423ec90ec374153366) feat(ui): animate toasts
- [415be50](https://git.max-richter.dev/max/nodarium/commit/415be50ae08477ed00d809ce18389b33fc665bb7) feat(ui): show toast after copying jsontree node
- [f0f4c00](https://git.max-richter.dev/max/nodarium/commit/f0f4c001379d1b1dd8ac4c7293e683a78eac85a0) feat: use beforeunload to make sure graph is saved
- [3c5f897](https://git.max-richter.dev/max/nodarium/commit/3c5f897b267e7d770259f05ac101cc5dec679d3d) feat: bunch of small fixes
- [ed0c470](https://git.max-richter.dev/max/nodarium/commit/ed0c47068a68bed712ee667d5ed376fd23d3de5c) fix: dispose unused geometry
- [a039bdd](https://git.max-richter.dev/max/nodarium/commit/a039bddba18a6e3ba6029332f77e44164f67b2ae) feat: debounce cameraPosition saving
- [5fa9d36](https://git.max-richter.dev/max/nodarium/commit/5fa9d36b348037f9a528f64c9af0c0420a6be7e9) feat: debounce box selection for performance
- [7d788f7](https://git.max-richter.dev/max/nodarium/commit/7d788f7e195aa9c1172840501b116e55522cf56f) feat: surface error when dropping wasm node
- [bd6dfeb](https://git.max-richter.dev/max/nodarium/commit/bd6dfeb4669e2c5f3cb555f37cb8307fbe9e9a93) fix: remove unused svelte-ignore comment
- [36f02ca](https://git.max-richter.dev/max/nodarium/commit/36f02cabd35a4b906eb85840bba770df151dac01) chore: format
- [3a78ad5](https://git.max-richter.dev/max/nodarium/commit/3a78ad5ee3121add86b69114e70d5981ac55d83f) docs: add ux guidelines
- [9a7a716](https://git.max-richter.dev/max/nodarium/commit/9a7a7166b79ab4c980a1830b2359556fb479d41d) fix: pasting nodes
- [4aff387](https://git.max-richter.dev/max/nodarium/commit/4aff3874d35450b9e7fd1be74be47f0cf4101c5e) feat: show toast on some errors
- [f415eda](https://git.max-richter.dev/max/nodarium/commit/f415edab57081778bef957356ea6585173123d59) fix: automatically update graph background on theme swtich
- [7439596](https://git.max-richter.dev/max/nodarium/commit/743959639f33298f9679d3d2433be3c28af40ba5) feat: no results state in add menu
- [d9b8b36](https://git.max-richter.dev/max/nodarium/commit/d9b8b3668664b23785835c7fae96cdd2db630b71) feat: update some inputs
- [ebf1396](https://git.max-richter.dev/max/nodarium/commit/ebf13967a4adb258abdc270437d988cd74b50b50) feat: use new button component everyhwerre
- [a4f51ef](https://git.max-richter.dev/max/nodarium/commit/a4f51efeaddb02e7f59963b1be0e01c5312b30fa) feat: add spinning icon
- [308626b](https://git.max-richter.dev/max/nodarium/commit/308626bcdc6b6162c393d0c6f3ac00fbb4789e5d) feat: add toast component
- [73155dc](https://git.max-richter.dev/max/nodarium/commit/73155dcb46586e1e65be0b85fc1061a3377fb26f) feat: show confirm when deleting project
- [84afd15](https://git.max-richter.dev/max/nodarium/commit/84afd15746faf76e271c97cd2e99d72a03d2d152) feat: add #id to group name when duplicates exist
- [af40db3](https://git.max-richter.dev/max/nodarium/commit/af40db3386b5f55321850ff405e550ade82132ba) feat: add ZoomIndicator to graph

# v0.0.6 (2026-05-05)

## Features

- Upgrade graph source panel and JSON viewer with click-to-copy and improved select handling.
- Capture system stats in benchmark runs.
- Split CI into unit and end-to-end pipelines.
- Add `LLM.md` documentation.
- Add 🍀 Planty Tutorial Helper
- Add a way to group nodes

## Node Groups

Node Groups introduce a way to structure graphs into nested, navigable subgraphs for better organization and reuse.

- Nested graph workflows with full runtime support
- Group input and output nodes defining clear boundaries
- Named groups for organization and identification
- Breadcrumb navigation for navigating nested levels
- Context-aware UI when editing inside groups
- Reliable enter/exit transitions with state restoration
- Ability to ungroup nodes back into the main graph

## Planty

Planty is a Clippy-inspired in-app tutorial and guidance system that helps users understand and use the graph editor through contextual assistance.

- Context-aware hints based on user actions in the graph
- Step-by-step onboarding flows for core features
- Inline guidance tied to nodes, sockets, and interactions
- Lightweight runtime integration without external build requirements

## Fixes

- Fix benchmark execution and CI integration issues
- Resolve debug node ID mismatch
- Fix ESLint, TypeScript, Playwright, and test synchronization issues
- Fix packaging issues in `@nodarium/planty` and UI library
- Restore correct graph references after exiting node groups

## Refactors

- Remove graph element handling from `graphManager`
- Restrict panel rendering to selected nodes only
- Move JSON viewer into shared UI package
- Clean up CI workflows

## Chores

- Upgrade dependencies via `pnpm upgrade`
- Add SvelteKit sync before E2E tests
- Remove flaky screenshot artifacts
- General formatting, linting, and test maintenance

---

- [82c2f08](https://git.max-richter.dev/max/nodarium/commit/82c2f08a5653ccd596c6982a1d9efa4b20a0b624) chore: cleanup changelog
- [a00db40](https://git.max-richter.dev/max/nodarium/commit/a00db400bba909db5da499e8484d6ed5541c4ad7) fix: dont crash when no groups exist
- [2d9eb0c](https://git.max-richter.dev/max/nodarium/commit/2d9eb0c0879611c2355e52100b150dd39b924684) fix: make planty work
- [954f572](https://git.max-richter.dev/max/nodarium/commit/954f5726c305508329856b6707a41b77f297c4bf) Merge pull request 'feat: initial node groups' (#44) from feat/group-node-own into main
- [63d5b80](https://git.max-richter.dev/max/nodarium/commit/63d5b8079d785b4fe1e689611cd38e5dd4d3ecb6) chore: pnpm format
- [3e32ca4](https://git.max-richter.dev/max/nodarium/commit/3e32ca419a1b914cbe64d98d887a89f4f5abb0e2) feat: ungroup nodes
- [f0cb12a](https://git.max-richter.dev/max/nodarium/commit/f0cb12a088609f5bd56096f7c1f244af0a1f91d8) chore: fix some type issues
- [1d60090](https://git.max-richter.dev/max/nodarium/commit/1d60090ffe1a93af4c0df6f36741957ac13b1a73) chore: fixup graph manager tests
- [5b55056](https://git.max-richter.dev/max/nodarium/commit/5b55056fc12271b87393cf9ef3b61cdf518a9857) chore: remove graph element in graphManager
- [e2c2b1a](https://git.max-richter.dev/max/nodarium/commit/e2c2b1a4d73e0953ddd0125cabe83649b58c8996) chore: remove e2e test screenshots (too flaky)
- [7f082ad](https://git.max-richter.dev/max/nodarium/commit/7f082ad8f6f144b92947000b91ceddce3c52704b) feat: implement node sockets ui
- [ed11195](https://git.max-richter.dev/max/nodarium/commit/ed1119532750d47a1395ced92fe310a6aa5e070e) chore: refactor graphStack to be simpler
- [8ad62cf](https://git.max-richter.dev/max/nodarium/commit/8ad62cfc8e8c60e4e72fdaa3d8caca75a7472da6) feat: add node group breadcrumbs
- [bff140a](https://git.max-richter.dev/max/nodarium/commit/bff140a764ff0b6ed2c6e42814a088ca9e2ef1ee) feat: show different ui when inside group
- [85e2fd1](https://git.max-richter.dev/max/nodarium/commit/85e2fd1a7126aab4a544eea3d185cfc63754cb73) fix: use correct id for debug node
- [5beb031](https://git.max-richter.dev/max/nodarium/commit/5beb03196d46434f617442cc55d721c7d3eebe33) fix: broken format command for @nodarium/planty
- [83e0e47](https://git.max-richter.dev/max/nodarium/commit/83e0e47082ed0d9fbd60e67a6d6c9ed65a9f4f24) refactor: only show group/node panel when selected
- [106797d](https://git.max-richter.dev/max/nodarium/commit/106797de32f0d57059c481770e734b555900421d) feat: make group input/output node work
- [1a56ba9](https://git.max-richter.dev/max/nodarium/commit/1a56ba986d761be545c2cf0236f9c374222da6b1) damn dude
- [703f531](https://git.max-richter.dev/max/nodarium/commit/703f531cd370c2d440a696a68413ea6d5f54019f) chore: make eslint and playwright happy
- [0ed22f2](https://git.max-richter.dev/max/nodarium/commit/0ed22f20b913837b8e90d45aa1a4ada1a90a9313) chore: pnpm upgrade
- [733b0a2](https://git.max-richter.dev/max/nodarium/commit/733b0a2ceb3be6fac1eda5bbf856a4bd1f724e36) chore: sync sveltekit app before e2e
- [8f60816](https://git.max-richter.dev/max/nodarium/commit/8f60816c7841c845120e71590e1c4672d218703e) chore: sync sveltekit app before e2e
- [cd7b51d](https://git.max-richter.dev/max/nodarium/commit/cd7b51d86a1243e0714d9be73588db56c844086c) chore: sync sveltekit app before e2e
- [6c9cd15](https://git.max-richter.dev/max/nodarium/commit/6c9cd1505d72c1c0600910f56ee158d07b7e4811) chore: sync sveltekit app before e2e
- [db5ee8b](https://git.max-richter.dev/max/nodarium/commit/db5ee8ba29812e6865706966ff690ef335ff5e4f) fix: make eslint happy
- [a6b9ca4](https://git.max-richter.dev/max/nodarium/commit/a6b9ca43155b5e9357570705b6f45ba4fc3490a8) feat: capture system stats in benchmark
- [d4910ab](https://git.max-richter.dev/max/nodarium/commit/d4910aba8c5408fb9321b1a22d4c35eae8142309) chore: pnpm format
- [e695c76](https://git.max-richter.dev/max/nodarium/commit/e695c7649015b88d8ca6662bfee5dc0329dd89ac) chore: make eslint happy
- [2a54fa7](https://git.max-richter.dev/max/nodarium/commit/2a54fa7590c1834904a45fb0fc1fd0aa371f0120) feat: add name to groups
- [6d5cac6](https://git.max-richter.dev/max/nodarium/commit/6d5cac65e8acf013ac0a4a61baf3bbc423252d53) feat(ui): click-to-copy on node values in jsonviewer
- [3ee074b](https://git.max-richter.dev/max/nodarium/commit/3ee074b11c9bc216ce7221f5611c8177cef9b313) feat(ui): make inputselect also handle value+label options
- [59a1e63](https://git.max-richter.dev/max/nodarium/commit/59a1e63396635d05543fa4636de5800ce4899a2a) feat: add unit tests for graph state
- [317d155](https://git.max-richter.dev/max/nodarium/commit/317d1552cea5a234ab1ab87684be9a3724b1976f) fix: graph correctly restore html refs after exiting node group
- [78439b1](https://git.max-richter.dev/max/nodarium/commit/78439b19e96e6dbdfb31cc794f5b6e1ff005e26e) fix: make benchmark work
- [ef217b1](https://git.max-richter.dev/max/nodarium/commit/ef217b1c409c25d6054515e1f705831ddbf5a24b) feat: some updates
- [7499b80](https://git.max-richter.dev/max/nodarium/commit/7499b80789e99b87df54d6891597fac7edb56b0f) fix: make the runtime work with groups
- [a5b663f](https://git.max-richter.dev/max/nodarium/commit/a5b663f6fc5e67d5ef67b128c22cdc7363232de5) feat(ci): split e2e and unit tests
- [0550670](https://git.max-richter.dev/max/nodarium/commit/05506704bf68dfd289a1c5130d5d86ddd98954b1) feat: let claude fix ci
- [63188e5](https://git.max-richter.dev/max/nodarium/commit/63188e57fd2cf055161508ee88cbef0cd941e662) feat: let claude fix ci
- [4572d30](https://git.max-richter.dev/max/nodarium/commit/4572d30005d3538f357d4e9f1fb637c1a6559fb1) feat: let claude refactor ci
- [ccc376d](https://git.max-richter.dev/max/nodarium/commit/ccc376d158f209b85a62d98919ac716e46e3e253) feat: store total vertices/faces in benchmarkl
- [7e432e9](https://git.max-richter.dev/max/nodarium/commit/7e432e9033f7fdc73c21bb363cf502c1d8085407) chore: update ci workflow
- [01f5837](https://git.max-richter.dev/max/nodarium/commit/01f58377c21048f95c839504a3e8c46c27ba12ae) feat: make more node group features work
- [6ef5dc2](https://git.max-richter.dev/max/nodarium/commit/6ef5dc28ed9a4874a7b5fb2a9dca73efd1632519) chore: move jsonviewer into ui package
- [3450d70](https://git.max-richter.dev/max/nodarium/commit/3450d7004781ea58f61d563967441a251c817a9b) docs: add LLM.md
- [731b9e9](https://git.max-richter.dev/max/nodarium/commit/731b9e9b1e52598e11044874a46976e517a1150c) feat: upgrade graph source panel
- [72f07d0](https://git.max-richter.dev/max/nodarium/commit/72f07d0a501fa715c40cf0e7c17527ef3f98e96b) feat: initial node groups
- [a56e8f4](https://git.max-richter.dev/max/nodarium/commit/a56e8f445edb6064ae7a7b3b783fb7445f1b4e69) feat(ci): install openssh client
- [1257274](https://git.max-richter.dev/max/nodarium/commit/12572742eb3ba1641cc744a18d330e88df50e9d0) fix(planty): remove debug span
- [7aa9979](https://git.max-richter.dev/max/nodarium/commit/7aa9979e355fcf90342e8b5f1d233b879bb6c71f) chore: update e2e tests
- [fc35a68](https://git.max-richter.dev/max/nodarium/commit/fc35a68826885ac7e9c624b39a5c0fe7d1cb83f0) fix: dont package ui library
- [aba6f03](https://git.max-richter.dev/max/nodarium/commit/aba6f03bcce3e4363f0f22337d0000083bfff9a9) fix: dont package ui library
- [2d6fd00](https://git.max-richter.dev/max/nodarium/commit/2d6fd00fd1ba31bfa943b6d3a4a628bd5132f668) fix: dont package ui library
- [d231946](https://git.max-richter.dev/max/nodarium/commit/d231946e50975dfa1b41696cefbbc2f742480ea8) fix: remove unused imports
- [e2f4a24](https://git.max-richter.dev/max/nodarium/commit/e2f4a24f759b917d3c7c1ca0b8347312785a03e5) fix(planty): make sure config is completely static
- [58d39cd](https://git.max-richter.dev/max/nodarium/commit/58d39cd101298e6a41922d18466899f7bf4a0f97) feat: improve planty ux
- [7ebb129](https://git.max-richter.dev/max/nodarium/commit/7ebb1297ac75987b3348dd81a57e0d25ed0a7405) feat(app): make zoom in nicer
- [23f65a1](https://git.max-richter.dev/max/nodarium/commit/23f65a1c63650faba2051a2c87f7625378b4b0c6) fix: remove unused header div
- [acdc582](https://git.max-richter.dev/max/nodarium/commit/acdc582e957df149ab723d268ac2e205595db199) feat: use ui and planty without build
- [7a3e9eb](https://git.max-richter.dev/max/nodarium/commit/7a3e9eb893182e46e72e203df1eb7532a4652ddd) chore: update test screenshot
- [be82312](https://git.max-richter.dev/max/nodarium/commit/be82312ea049b21e5ff859163e54ba6da88328a0) chore: update test screenshot
- [84f67e9](https://git.max-richter.dev/max/nodarium/commit/84f67e9c33a1141d7e0c3332375576cd0898a47a) fix: update planty types
- [491e345](https://git.max-richter.dev/max/nodarium/commit/491e345c2ff1893916848380e8941fa77dff44ec) feat: build planty in post install
- [ba501b2](https://git.max-richter.dev/max/nodarium/commit/ba501b211db1d70930fa461e1fd82abb5b639c00) fix: correct tsconfig for planty
- [7d76b9e](https://git.max-richter.dev/max/nodarium/commit/7d76b9e1f77ab934289bb18df6197bfd58ce3eeb) fix: mark planty as type:module
- [5d4e2e9](https://git.max-richter.dev/max/nodarium/commit/5d4e2e928093cac39192960d9de21a0e1710904e) fix: make formatter happy
- [4de15b1](https://git.max-richter.dev/max/nodarium/commit/4de15b19c8bdb35e53ba0d3e3e459cdbb12aee9d) feat: wire up planty with nodarium/app
- [168e6fc](https://git.max-richter.dev/max/nodarium/commit/168e6fcc19dc55383b0b21d2b3ebab733058fb94) feat: update some node default settings
- [c0eb75d](https://git.max-richter.dev/max/nodarium/commit/c0eb75d53c4251d041744b19a37c44d0d4a1728c) feat: new planty package
- [2ec9bfc](https://git.max-richter.dev/max/nodarium/commit/2ec9bfc3c96a97aaf29557c70f76b7bf08156e15) feat(ci): compress benchmark data
- [c975206](https://git.max-richter.dev/max/nodarium/commit/c97520617a0d48a765544feebf2d510400db8fb8) fix(ci): use older upload-artifact action
- [6475790](https://git.max-richter.dev/max/nodarium/commit/64757901766efb7ccbd3693ebc63ba1367fe6d88) fix(ci): build nodes before benchmarking
- [580ec73](https://git.max-richter.dev/max/nodarium/commit/580ec7346599e5d538ff53f31808ff770b4a8095) ci: run benchmark in ci

# v0.0.5 (2026-02-13)

## Features

- Implement debug node with full runtime integration, wildcard (`*`) inputs, variable-height nodes and parameters, and a quick-connect shortcut.
- Add color-coded node sockets and edges to visually indicate data types.
- Recursively merge `localState` with the initial state to safely handle outdated settings stored in `localStorage` when the settings schema changes.
- Clamp the Add Menu to the viewport.
- Add application favicon.
- Consolidate all developer settings into a single **Advanced Mode** setting.

## Fixes

- Fix InputNumber arrow visibility in the light theme.
- Correct changelog formatting issues.

## Chores

- Add `pnpm qa` pre-commit command.
- Run linting and type checks before build in CI.
- Sign release commits with a PGP key.
- General formatting, lint/type cleanup, test snapshot updates, and `.gitignore` maintenance.

---

- [f16ba26](https://git.max-richter.dev/max/nodarium/commit/f16ba2601ff0e8f0f4454e24689499112a2a257a) fix(ci): still trying to get gpg to work
- [cc6b832](https://git.max-richter.dev/max/nodarium/commit/cc6b832f1576356e5453ee4289b02f854152ff9a) fix(ci): trying to get gpg to work
- [dd5fd5b](https://git.max-richter.dev/max/nodarium/commit/dd5fd5bf1715d371566bd40419b72ca05e63401e) fix(ci): better add updates to package.json
- [38d0fff](https://git.max-richter.dev/max/nodarium/commit/38d0fffcf4ca0a346857c3658ccefdfcdf16e217) chore: update ci image
- [bce06da](https://git.max-richter.dev/max/nodarium/commit/bce06da456e3c008851ac006033cfff256015a47) ci: add gpg-agent to ci image
- [af585d5](https://git.max-richter.dev/max/nodarium/commit/af585d56ec825662961c8796226ed9d8cb900795) feat: use new ci image with gpg
- [0aa73a2](https://git.max-richter.dev/max/nodarium/commit/0aa73a27c1f23bea177ecc66034f8e0384c29a8e) feat: install gpg in ci image
- [c1ae702](https://git.max-richter.dev/max/nodarium/commit/c1ae70282cb5d58527180614a80823d80ca478c5) feat: add color to sockets
- [4c7b03d](https://git.max-richter.dev/max/nodarium/commit/4c7b03dfb82174317d8ba01f4725af804201154d) feat: add gradient mesh line
- [144e8cc](https://git.max-richter.dev/max/nodarium/commit/144e8cc797cfcc5a7a1fd9a0a2098dc99afb6170) fix: correctly highlight possible outputs
- [12ff9c1](https://git.max-richter.dev/max/nodarium/commit/12ff9c151873d253ed2e54dcf56aa9c9c4716c7c) Merge pull request 'feat/debug-node' (#41) from feat/debug-node into main
- [8d3ffe8](https://git.max-richter.dev/max/nodarium/commit/8d3ffe84ab9ca9e6d6d28333752e34da878fd3ea) Merge branch 'main' into feat/debug-node
- [95ec93e](https://git.max-richter.dev/max/nodarium/commit/95ec93eeada9bf062e01e1e77b67b8f0343a51bf) feat: better handle ctrl+shift clicks and selections
- [d39185e](https://git.max-richter.dev/max/nodarium/commit/d39185efafc360f49ab9437c0bad1f64665df167) feat: add "pnpm qa" command to check before commit
- [81580cc](https://git.max-richter.dev/max/nodarium/commit/81580ccd8c1db30ce83433c4c4df84bd660d3460) fix: cleanup some type errors
- [bf6f632](https://git.max-richter.dev/max/nodarium/commit/bf6f632d2772c3da812d5864c401f17e1aa8666a) feat: add shortcut to quick connect to debug
- [e098be6](https://git.max-richter.dev/max/nodarium/commit/e098be60135f57cf863904a58489e032ed27e8b4) fix: also execute all nodes before debug node
- [ec13850](https://git.max-richter.dev/max/nodarium/commit/ec13850e1c0ca5846da614d25887ff492cf8be04) fix: make debug node work with runtime
- [15e08a8](https://git.max-richter.dev/max/nodarium/commit/15e08a816339bdf9de9ecb6a57a7defff42dbe8c) feat: implement debug node
- [48cee58](https://git.max-richter.dev/max/nodarium/commit/48cee58ad337c1c6c59a0eb55bf9b5ecd16b99d0) chore: update test snapshots
- [3235cae](https://git.max-richter.dev/max/nodarium/commit/3235cae9049e193c242b6091cee9f01e67ee850e) chore: fix lint and typecheck errors
- [3f44072](https://git.max-richter.dev/max/nodarium/commit/3f440728fc8a94d59022bb545f418be049a1f1ba) feat: implement variable height for node shader
- [da09f8b](https://git.max-richter.dev/max/nodarium/commit/da09f8ba1eda5ed347433d37064a3b4ab49e627e) refactor: move debug node into runtime
- [ddc3b4c](https://git.max-richter.dev/max/nodarium/commit/ddc3b4ce357ef1c1e8066c0a52151713d0b6ed95) feat: allow variable height node parameters
- [2690fc8](https://git.max-richter.dev/max/nodarium/commit/2690fc871291e73d3d028df9668e8fffb1e77476) chore: gitignore pnpm-store
- [072ab90](https://git.max-richter.dev/max/nodarium/commit/072ab9063ba56df0673020eb639548f3a8601e04) feat: add initial debug node
- [e23cad2](https://git.max-richter.dev/max/nodarium/commit/e23cad254d610e00f196b7fdb4532f36fd735a4b) feat: add "*" datatype for inputs for debug node
- [5b5c63c](https://git.max-richter.dev/max/nodarium/commit/5b5c63c1a9c4ef757382bd4452149dc9777693ff) fix(ui): make arrows on inputnumber visible on lighttheme
- [c9021f2](https://git.max-richter.dev/max/nodarium/commit/c9021f2383828f2e2b5594d125165bbc8f70b8e7) refactor: merge all dev settings into one setting
- [9eecdd4](https://git.max-richter.dev/max/nodarium/commit/9eecdd4fb85dc60b8196101050334e26732c9a34) Merge pull request 'feat: merge localState recursively with initial' (#38) from feat/debug-node into main
- [7e71a41](https://git.max-richter.dev/max/nodarium/commit/7e71a41e5229126d404f56598c624709961dbf3b) feat: merge localState recursively with initial
- [07cd9e8](https://git.max-richter.dev/max/nodarium/commit/07cd9e84eb51bc02b7fed39c36cf83caba175ad7) feat: clamp AddMenu to viewport
- [a31a49a](https://git.max-richter.dev/max/nodarium/commit/a31a49ad503d69f92f2491dd685729060ea49896) ci: lint and typecheck before build
- [850d641](https://git.max-richter.dev/max/nodarium/commit/850d641a25cd0c781478c58c117feaf085bdbc62) chore: pnpm format
- [ee5ca81](https://git.max-richter.dev/max/nodarium/commit/ee5ca817573b83cacfa3709e0ae88c6263bc39c1) ci: sign release commits with pgp key
- [22a1183](https://git.max-richter.dev/max/nodarium/commit/22a11832b861ae8b44e2d374b55d12937ecab247) fix(ci): correctly format changelog
- [b5ce572](https://git.max-richter.dev/max/nodarium/commit/b5ce5723fa4a35443df39a9096d0997f808f0b4f) chore: format favicon svg
- [102130c](https://git.max-richter.dev/max/nodarium/commit/102130cc7777ceebcdb3de8466c90cef5b380111) feat: add favicon
- [1668a2e](https://git.max-richter.dev/max/nodarium/commit/1668a2e6d59db071ab3da45204c2b7bfcd2150a2) chore: format changelog.md

# v0.0.4 (2026-02-10)

## Features

- Added shape and leaf nodes, including rotation support.
- Added high-contrast light theme and improved overall node readability.
- Enhanced UI with dots background, clearer details, and consistent node coloring.
- Improved changelog display and parsing robustness.

## Fixes

- Fixed UI issues (backside rendering, missing types, linter errors).
- Improved CI handling of commit messages and changelog placement.

## Chores

- Simplified CI quality checks.
- Updated dprint linters.
- Refactored changelog code.

---

- [51de3ce](https://git.max-richter.dev/max/nodarium/commit/51de3ced133af07b9432e1137068ef43ddfecbc9) fix(ci): update changelog before building
- [8d403ba](https://git.max-richter.dev/max/nodarium/commit/8d403ba8039a05b687f050993a6afca7fb743e12) Merge pull request 'feat/shape-node' (#36) from feat/shape-node into main
- [6bb3011](https://git.max-richter.dev/max/nodarium/commit/6bb301153ac13c31511b6b28ae95c6e0d4c03e9e) Merge remote-tracking branch 'origin/main' into feat/shape-node
- [02eee5f](https://git.max-richter.dev/max/nodarium/commit/02eee5f9bf4b1bc813d5d28673c4d5d77b392a92) fix: disable macro logs in wasm
- [4f48a51](https://git.max-richter.dev/max/nodarium/commit/4f48a519a950123390530f1b6040e2430a767745) feat(nodes): add rotation to instance node
- [97199ac](https://git.max-richter.dev/max/nodarium/commit/97199ac20fb079d6c157962d1a998d63670d8797) feat(nodes): implement leaf node
- [f36f0cb](https://git.max-richter.dev/max/nodarium/commit/f36f0cb2305692c7be60889bcde7f91179e18b81) feat(ui): show circles only when hovering InputShape
- [ed3d48e](https://git.max-richter.dev/max/nodarium/commit/ed3d48e07fa6db84bbb24db6dbe044cbc36f049f) fix(runtime): correctly encode 2d shape for wasm nodes
- [c610d6c](https://git.max-richter.dev/max/nodarium/commit/c610d6c99152d8233235064b81503c2b0dc4ada8) fix(app): show backside in three instances
- [8865b9b](https://git.max-richter.dev/max/nodarium/commit/8865b9b032bdf5a1385b4e9db0b1923f0e224fdd) feat(node): initial leaf / shape nodes
- [235ee5d](https://git.max-richter.dev/max/nodarium/commit/235ee5d979fbd70b3e0fb6f09a352218c3ff1e6d) fix(app): wrong linter errors in changelog
- [23a4857](https://git.max-richter.dev/max/nodarium/commit/23a48572f3913d91d839873cc155a16139c286a6) feat(app): dots background for node interface
- [e89a46e](https://git.max-richter.dev/max/nodarium/commit/e89a46e146e9e95de57ffdf55b05d16d6fe975f4) feat(app): add error page
- [cefda41](https://git.max-richter.dev/max/nodarium/commit/cefda41fcf3d5d011c9f7598a4f3f37136602dbd) feat(theme): optimize node readability
- [21d0f0d](https://git.max-richter.dev/max/nodarium/commit/21d0f0da5a26492fa68ad4897a9b1d9e88857030) feat: add high-contrast-light theme
- [4620245](https://git.max-richter.dev/max/nodarium/commit/46202451ba3eea73bd1bc6ef5129b3e26ee9315c) ci: simplify ci quality checks
- [0f4239d](https://git.max-richter.dev/max/nodarium/commit/0f4239d179ddedd3d012ca98b5bc3312afbc8f10) ci: simplify ci quality checks
- [d9c9bb5](https://git.max-richter.dev/max/nodarium/commit/d9c9bb5234bc8776daf26be99ba77a2145c70649) fix(theme): allow raw html in head style
- [18802fd](https://git.max-richter.dev/max/nodarium/commit/18802fdc10294a58425f052a4fde4bcf4be58caf) fix(ui): add missing types
- [b1cbd23](https://git.max-richter.dev/max/nodarium/commit/b1cbd235420c99a11154ef6a899cc7e14faf1c37) feat(app): use same color for node outline and header
- [33f10da](https://git.max-richter.dev/max/nodarium/commit/33f10da396fdc13edcb8faaee212280102b24f3a) feat(ui): make details stand out
- [af5b3b2](https://git.max-richter.dev/max/nodarium/commit/af5b3b23ba18d73d6abec60949fb0c9edfc25ff8) fix: make sure that CHANGELOG.md is in correct place
- [64d75b9](https://git.max-richter.dev/max/nodarium/commit/64d75b9686c494075223a0a318297fe59ec99e81) feat(ui): add InputColor and custom theme
- [2e6466c](https://git.max-richter.dev/max/nodarium/commit/2e6466ceca1d2131581d1862e93c756affdf6cd6) chore: update dprint linters
- [20d8e2a](https://git.max-richter.dev/max/nodarium/commit/20d8e2abedf0de30299d947575afef9c8ffd61d9) feat(theme): improve light theme a bit
- [715e1d0](https://git.max-richter.dev/max/nodarium/commit/715e1d095b8a77feb0cf66bbb444baf0f163adcb) feat(theme): merge edge and connection color
- [07e2826](https://git.max-richter.dev/max/nodarium/commit/07e2826f16dafa6a07377c9fb591168fa5c2abcf) feat(ui): improve colors of input shape
- [e0ad97b](https://git.max-richter.dev/max/nodarium/commit/e0ad97b003fd8cb4d950c03e5488a5accf6a37d0) feat(ui): highlight circle on hover on InputShape
- [93df4a1](https://git.max-richter.dev/max/nodarium/commit/93df4a19ff816e2bdfa093594721f0829f84c9e6) fix(ci): handle newline in commit messages for git.json
- [d661a4e](https://git.max-richter.dev/max/nodarium/commit/d661a4e4a9dfa6c9c73b5e24a3edcf56e1bbf48c) feat(ui): improve InputShape ux
- [c7f808c](https://git.max-richter.dev/max/nodarium/commit/c7f808ce2d52925425b49f92edf49d9557f8901d) wip
- [72d6cd6](https://git.max-richter.dev/max/nodarium/commit/72d6cd6ea2886626823e6e86856f19338c7af3c1) feat(ui): add initial InputShape element
- [615f2d3](https://git.max-richter.dev/max/nodarium/commit/615f2d3c4866a9e85f3eca398f3f02100c4df355) feat(ui): allow custom snippets in ui section header
- [2fadb68](https://git.max-richter.dev/max/nodarium/commit/2fadb6802de640d692fdab7d654311df0d7b4836) refactor: make changelog code simpler
- [9271d3a](https://git.max-richter.dev/max/nodarium/commit/9271d3a7e4cb0cc751b635c2adb518de7b4100c7) fix(app): handle error while parsing commit
- [13c83ef](https://git.max-richter.dev/max/nodarium/commit/13c83efdb962a6578ade59f10cc574fef0e17534) fix(app): handle error while parsing changelog
- [e44b73b](https://git.max-richter.dev/max/nodarium/commit/e44b73bebfb1cc8e872cd2fa7d8b6ff3565df374) feat: optimize changelog display
- [979e9fd](https://git.max-richter.dev/max/nodarium/commit/979e9fd92289eba9f77221c563337c00028e4cf5) feat: improve changelog readbility
- [544500e](https://git.max-richter.dev/max/nodarium/commit/544500e7fe9ee14412cef76f3c7a32ba6f291656) chore: remove pgp from changelog
- [aaebbc4](https://git.max-richter.dev/max/nodarium/commit/aaebbc4bc082ee93c2317ce45071c9bc61b0b77e) fix: some stuff with ci

# v0.0.3 (2026-02-07)

## Features

- Edge dragging now highlights valid connection sockets, improving graph editing clarity.
- InputNumber supports snapping to predefined values while holding Alt.
- Changelog is accessible directly from the sidebar and now includes git metadata and a list of commits.

## Fixes

- Fixed incorrect socket highlighting when an edge already existed.
- Corrected initialization of `InputNumber` values outside min/max bounds.
- Fixed initialization of nested vec3 inputs.
- Multiple CI fixes to ensure reliable builds, correct environment variables, and proper image handling.

## Maintenance / CI

- Significant CI and Dockerfile cleanup and optimization.
- Improved git metadata generation during builds.
- Dependency updates, formatting, and test snapshot updates.

---

- [f8a2a95](https://git.max-richter.dev/max/nodarium/commit/f8a2a95bc18fa3c8c1db67dc0c2b66db1ff0d866) chore: clean CHANGELOG.md
- [c9dd143](https://git.max-richter.dev/max/nodarium/commit/c9dd143916d758991f3ba30723a32c18b6f98bb5) fix(ci): correctly add release notes from tag to changelog
- [898dd49](https://git.max-richter.dev/max/nodarium/commit/898dd49aee930350af8645382ef5042765a1fac7) fix(ci): correctly copy changelog to build output
- [9fb69d7](https://git.max-richter.dev/max/nodarium/commit/9fb69d760fdf92ecc2448e468242970ec48443b0) feat: show commits since last release in changelog
- [bafbcca](https://git.max-richter.dev/max/nodarium/commit/bafbcca2b8a7cd9f76e961349f11ec84d1e4da63) fix: wrong socket was highlighted when dragging node
- [8ad9e55](https://git.max-richter.dev/max/nodarium/commit/8ad9e5535cd752ef111504226b4dac57b5adcf3d) feat: highlight possible sockets when dragging edge
- [11eaeb7](https://git.max-richter.dev/max/nodarium/commit/11eaeb719be7f34af8db8b7908008a15308c0cac) feat(app): display some git metadata in changelog
- [74c2978](https://git.max-richter.dev/max/nodarium/commit/74c2978cd16d2dd95ce1ae8019dfb9098e52b4b6) chore: cleanup git.json a bit
- [4fdc247](https://git.max-richter.dev/max/nodarium/commit/4fdc24790490d3f13ee94a557159617f4077a2f9) ci: update build.sh to correct git.json
- [c3f8b4b](https://git.max-richter.dev/max/nodarium/commit/c3f8b4b5aad7a525fb11ab14c9236374cb60442d) ci: debug available env vars
- [67591c0](https://git.max-richter.dev/max/nodarium/commit/67591c0572b873d8c7cd00db8efb7dac2d6d4de2) chore: pnpm format
- [de1f9d6](https://git.max-richter.dev/max/nodarium/commit/de1f9d6ab669b8e699d98b8855e125e21030b5b3) feat(ui): change inputnumber to snap to values when alt is pressed
- [6acce72](https://git.max-richter.dev/max/nodarium/commit/6acce72fb8c416cc7f6eec99c2ae94d6529e960c) fix(ui): correctly initialize InputNumber
- [cf8943b](https://git.max-richter.dev/max/nodarium/commit/cf8943b2059aa286e41865caf75058d35498daf7) chore: pnpm update
- [9e03d36](https://git.max-richter.dev/max/nodarium/commit/9e03d36482bb4f972c384b66b2dcf258f0cd18be) chore: use newest ci image
- [fd7268d](https://git.max-richter.dev/max/nodarium/commit/fd7268d6208aede435e1685817ae6b271c68bd83) ci: make dockerfile work
- [6358c22](https://git.max-richter.dev/max/nodarium/commit/6358c22a853ec340be5223fabb8289092e4f4afe) ci: use tagged own image for ci
- [655b6a1](https://git.max-richter.dev/max/nodarium/commit/655b6a18b282f0cddcc750892e575ee6c311036b) ci: make dockerfile work
- [37b2bdc](https://git.max-richter.dev/max/nodarium/commit/37b2bdc8bdbd8ded6b22b89214b49de46f788351) ci: update ci Dockerfile to work
- [94e01d4](https://git.max-richter.dev/max/nodarium/commit/94e01d4ea865f15ce06b52827a1ae6906de5be5e) ci: correctly build and push ci image
- [35f5177](https://git.max-richter.dev/max/nodarium/commit/35f5177884b62bbf119af1bbf4df61dd0291effb) feat: try to optimize the Dockerfile
- [ac2c61f](https://git.max-richter.dev/max/nodarium/commit/ac2c61f2211ba96bbdbb542179905ca776537cec) ci: use actual git url in ci
- [ef3d462](https://git.max-richter.dev/max/nodarium/commit/ef3d46279f4ff9c04d80bb2d9a9e7cfec63b224e) fix(ci): build before testing
- [703da32](https://git.max-richter.dev/max/nodarium/commit/703da324fabbef0e2c017f0f7a925209fa26bd03) ci: automatically build ci image and store locally
- [1dae472](https://git.max-richter.dev/max/nodarium/commit/1dae472253ccb5e3766f2270adc053b922f46738) ci: add a git.json metadata file during build
- [09fdfb8](https://git.max-richter.dev/max/nodarium/commit/09fdfb88cd203ace0e36663ebdb2c8c7ba53f190) chore: update test screenshots
- [04b63cc](https://git.max-richter.dev/max/nodarium/commit/04b63cc7e2fc4fcfa0973cf40592d11457179db3) feat: add changelog to sidebar
- [cb6a356](https://git.max-richter.dev/max/nodarium/commit/cb6a35606dfda50b0c81b04902d7a6c8e59458d2) feat(ci): also cache cargo stuff
- [9c9f3ba](https://git.max-richter.dev/max/nodarium/commit/9c9f3ba3b7c94215a86b0a338a5cecdd87b96b28) fix(ci): use GITHUB_instead of GITEA_ for env vars
- [08dda2b](https://git.max-richter.dev/max/nodarium/commit/08dda2b2cb4d276846abe30bc260127626bb508a) chore: pnpm format
- [059129a](https://git.max-richter.dev/max/nodarium/commit/059129a738d02b8b313bb301a515697c7c4315ac) fix(ci): deploy prs and main
- [437c9f4](https://git.max-richter.dev/max/nodarium/commit/437c9f4a252125e1724686edace0f5f006f58439) feat(ci): add list of all commits to changelog entry
- [48bf447](https://git.max-richter.dev/max/nodarium/commit/48bf447ce12949d7c29a230806d160840b7847e1) docs: straighten up changelog a bit
- [548fa4f](https://git.max-richter.dev/max/nodarium/commit/548fa4f0a1a14adc40a74da1182fa6da81eab3df) fix(app): correctly initialize vec3 inputs in nestedsettings

# v0.0.2 (2026-02-04)

## Fixes

---

- []() fix(ci): actually deploy on tags
- []() fix(app): correctly handle false value in settings

# v0.0.1 (2026-02-03)

chore: format
