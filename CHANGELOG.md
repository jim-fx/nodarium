# v0.0.9 (2026-11-03)

## Features

- Nodes can have multiple outputs, and the debug node steps through each of them.
- Graphs carry a version and old graphs are migrated when they load (#78).
- Length inputs read typed units like 10cm or 5mm, and plant templates are scaled to their real size.
- New projects start with a minimal stem graph, created directly from New project (#96).
- Show gizmos, human and arrow helpers and 3D text that flips to stay readable in the 3D view (#97).
- Fit the 3D camera to the viewport and store the 3D and node graph camera per project.
- Add a mute node (#64) and a /template route to inspect templates with vertex and face counts.
- Add a roadmap tab to the changelog panel (#80).

## Plantarium Nodes

- Points data type with points_on_path (#81), branch_from_points (#82) and instance_from_points (#83)
- to_geometry bakes paths into geometry (#74), with a variants mode and a resolution limit for baked paths (#104)
- The instance node accepts a list of geometries and picks them by weight or by index (#75)
- Umbel node for flat-topped flower clusters (#73)
- trunk_kink (#70), random_break (#61), avoid (#59), snap, apply_force and path join nodes
- Leaves get a thickness input and extruded paths have closed ends
- Paths encode their creator and a stable hashed id
- All node inputs are named in snake_case (#84)

## Plant Templates

- Add devil's backbone, japanese knotweed and wild carrot templates
- The palm grows coconuts (#105) and the grass picks its blades from a list of geometries (#109)
- Retune the willow and maidenhair, rebuild the daisy and drop the banana
- The default plant is scaled down (#95)

## Performance

- Skip far apart segments in the avoid node
- Key pooled meshes by path, cache bounds and hash geometry faster

## Fixes

- Fix the obj and glb exporter (#65)
- Run a node before the deepest node it feeds
- Keep the plant shape when resolution.curve changes
- Long node names no longer overflow the node header
- Drop instances scaled to nothing and accept a single geometry in the output node
- Handle an unconnected to_geometry input

## Chores

- Upgrade dependencies and rust crates, migrate to SvelteKit v3
- Rename all input.json files to definition.json
- Run checks, tests and deploy in a single CI job and keep builds and deploys in order across two runners
- Comment the branch preview link on pull requests

---

- [8a15424](https://git.max-richter.dev/max/nodarium/commit/8a15424b4c8e34068b8ba7efb5164485cfc1353d) Merge pull request 'feat: add the wild carrot template' (#112) from feat/wild-carrot-template into main
- [5bd2c25](https://git.max-richter.dev/max/nodarium/commit/5bd2c25a7c07dcce904f54ffadf0a9727fa41d6c) feat: add the wild carrot template
- [a1bcb94](https://git.max-richter.dev/max/nodarium/commit/a1bcb94794f914712360a39e8306444a2e5101ed) fix: hide the umbel's fine-tuning inputs
- [9218e29](https://git.max-richter.dev/max/nodarium/commit/9218e2917f21f03558a6ed00eca7b62ddbfc00c5) Merge pull request 'feat: use the geometry pick in the grass template' (#111) from feat/grass-geometry-pick into main
- [8623af4](https://git.max-richter.dev/max/nodarium/commit/8623af46ebc482b73c786de7d299c69c2bae5752) Merge pull request 'feat: add the umbel node' (#110) from feat/umbel-node into main
- [4b272d9](https://git.max-richter.dev/max/nodarium/commit/4b272d9d85ff5611f24facfa1e435cb147945655) feat: use the geometry pick in the grass template
- [096c34d](https://git.max-richter.dev/max/nodarium/commit/096c34d31fc2e13e6837d550d5d52410291afafe) feat: add the umbel node
- [c33fa42](https://git.max-richter.dev/max/nodarium/commit/c33fa427440ac96e422ee4cf1aa8a92b294de29c) Merge pull request 'feat: weighted and index-based geometry pick on instance' (#108) from feat/instance-weighted-pick into main
- [882615f](https://git.max-richter.dev/max/nodarium/commit/882615f68594746d25bec491fb842e74920ba05c) feat: weighted and index-based geometry pick on instance
- [61de531](https://git.max-richter.dev/max/nodarium/commit/61de531680f685e0b2cd52c60421339fa5d8ca16) Merge pull request 'feat: to_geometry variants mode and coconut palm' (#107) from feat/to-geometry-variants into main
- [a784ed2](https://git.max-richter.dev/max/nodarium/commit/a784ed2fed5dd151673393b590afe0f43deb0a5c) feat: coconuts on the palm template
- [651e649](https://git.max-richter.dev/max/nodarium/commit/651e649b10265197659143e8dae447fbfcc0e100) feat: to_geometry variants mode
- [73c05ec](https://git.max-richter.dev/max/nodarium/commit/73c05ec0802734c05902dc7de98527f4ed744a70) Merge pull request 'fix: geometry handling gaps around to_geometry' (#106) from fix/geometry-gaps into main
- [d229dcb](https://git.max-richter.dev/max/nodarium/commit/d229dcbb366ea49925f2698ae0df110e40d7ddbf) feat(to_geometry): limit the resolution of baked paths
- [d6a87c0](https://git.max-richter.dev/max/nodarium/commit/d6a87c081ea9b3e750986bb94761c6554a7d5155) fix(instance): drop instances scaled to nothing
- [b5c15b2](https://git.max-richter.dev/max/nodarium/commit/b5c15b21cb3929bc767ac5a3fdb89c376ce96e50) fix(output): accept a single geometry
- [85fa9d1](https://git.max-richter.dev/max/nodarium/commit/85fa9d1529870aefcbf92616e368452a0ea6b058) fix(utils): write the bracket count of created geometry like wrap_arg
- [0ecbe64](https://git.max-richter.dev/max/nodarium/commit/0ecbe6469640934b6f667bd19910b0a6ea4dbd6d) Merge pull request 'feat: instance node accepts a list of geometries' (#103) from feat/instance-geometry-list into main
- [1a926cc](https://git.max-richter.dev/max/nodarium/commit/1a926cc0e8ffeb7477861bfff8f1f8ceb1a084f8) feat: instance node accepts a list of geometries
- [a882d05](https://git.max-richter.dev/max/nodarium/commit/a882d05ad85b7ce252a283e43ede75873ce43666) Merge pull request 'feat: add the to_geometry node' (#102) from feat/to-geometry-node into main
- [6c0a00e](https://git.max-richter.dev/max/nodarium/commit/6c0a00e57c23f8f5b0f5efe302fef9d2124f8db4) fix: handle an unconnected to_geometry input
- [b390eec](https://git.max-richter.dev/max/nodarium/commit/b390eecdd018fc9d1671cb15a9fa3bf67d23c0d9) feat: add the to_geometry node
- [40e8a99](https://git.max-richter.dev/max/nodarium/commit/40e8a99e969c16e5d15609fc2d9719c49cb5a704) Merge pull request 'fix: scale down the default plant' (#101) from fix/smaller-default-plant into main
- [17ecf8a](https://git.max-richter.dev/max/nodarium/commit/17ecf8aad3756339f1f1abf0087b242c7c59c38d) Merge branch 'main' into fix/smaller-default-plant
- [a873dfe](https://git.max-richter.dev/max/nodarium/commit/a873dfeab31e70d8a85497b22de5c29b4b7e9ca9) fix: scale down the default plant
- [c73950a](https://git.max-richter.dev/max/nodarium/commit/c73950abaf481f79875905e7c5c030a534543a3a) Merge pull request 'feat: start new projects with a minimal stem graph' (#100) from feat/minimal-new-project into main
- [337c2ff](https://git.max-richter.dev/max/nodarium/commit/337c2ffe9686c09d9e8f85154c2ba871b7d0e0e5) Merge branch 'main' into feat/minimal-new-project
- [8a04658](https://git.max-richter.dev/max/nodarium/commit/8a0465809c656e23759c4494e5ad0966dd1f2d15) test: open the project browser via Open in the e2e test
- [b1ec80c](https://git.max-richter.dev/max/nodarium/commit/b1ec80cb23902f89045e37acbef751846778ec34) chore: rename general.clippy to general.planty setting
- [4636735](https://git.max-richter.dev/max/nodarium/commit/4636735cdba3d64ed6f8df731ba48a5cdd991765) chore: update default app settings
- [15f243c](https://git.max-richter.dev/max/nodarium/commit/15f243c803f594f535f161f7dc8461edace14bbe) feat: create a stem project directly from New project
- [842f3da](https://git.max-richter.dev/max/nodarium/commit/842f3dab07402837b29a49d92b4b0c20b93988df) Merge pull request 'Feat/zigzag templates' (#99) from feat/zigzag-templates into main
- [242fa13](https://git.max-richter.dev/max/nodarium/commit/242fa13a97ed0f052dc3748223060a0b3a0c323a) fix: keep default plant for the first project
- [2d3da3d](https://git.max-richter.dev/max/nodarium/commit/2d3da3d61022989a65dc9f28af2141f446144590) feat: start new projects with a minimal stem graph
- [19b2b61](https://git.max-richter.dev/max/nodarium/commit/19b2b616bccb79334b43d9be1b7e6de902091d17) Merge pull request 'feat: add 3D text that flips to stay readable' (#98) from feat/readable-text into main
- [a1cc726](https://git.max-richter.dev/max/nodarium/commit/a1cc7265eb7d410bbefa5ee13ed465416b29d8c8) feat: add 3D text that flips to stay readable
- [a11b197](https://git.max-richter.dev/max/nodarium/commit/a11b1970e4ad0e098d7db8bb818118ed6a6de84e) chore: update the daisy template
- [731df60](https://git.max-richter.dev/max/nodarium/commit/731df6092870c511f13811135b60308a21fe03aa) feat: preselect the matching template when pushing a graph
- [82301e9](https://git.max-richter.dev/max/nodarium/commit/82301e95f2098d7f5ab64e34e5afd34e5816b3f8) feat: add devil's backbone and japanese knotweed templates
- [7df02f4](https://git.max-richter.dev/max/nodarium/commit/7df02f4f208a405c0699353569e22d17124fe870) fix: run a node before the deepest node it feeds
- [e532366](https://git.max-richter.dev/max/nodarium/commit/e532366e3e469f9e4640ea85dbbb3271af5b003d) Merge pull request 'feat: add the instance_from_points node' (#92) from feat/instance-from-points-node into main
- [2fbd621](https://git.max-richter.dev/max/nodarium/commit/2fbd6216408bf8ec2ad67eefc1c724b150531264) Merge remote-tracking branch 'origin/main' into feat/instance-from-points-node
- [cd47a48](https://git.max-richter.dev/max/nodarium/commit/cd47a48b61ad91dbaab251153b35fc8510b733b9) Merge pull request 'fix: long node names overflow the node header' (#94) from fix/node-title-overflow into main
- [5b00257](https://git.max-richter.dev/max/nodarium/commit/5b0025752046d15d942bb68d5beec3019752d02f) Merge branch 'main' into fix/node-title-overflow
- [a97bc43](https://git.max-richter.dev/max/nodarium/commit/a97bc43b989b0057e8e48079a6eb32768b53d748) Merge pull request 'chore: remove AI filler comments and useless test code' (#93) from chore/ai-cleanup into main
- [07d2db7](https://git.max-richter.dev/max/nodarium/commit/07d2db7efd420db832537dbb9fd5aa67cb629e2b) chore: remove AI filler comments and useless test code
- [769974e](https://git.max-richter.dev/max/nodarium/commit/769974e06cd653710709646ce1d9f403bde56906) fix: long node names overflow the node header
- [9e62d1d](https://git.max-richter.dev/max/nodarium/commit/9e62d1d1afb535994ea63b382ee3426c9c5cf4f5) Merge pull request 'feat: add the branch_from_points node' (#91) from feat/branch-from-points-node into main
- [131e6ff](https://git.max-richter.dev/max/nodarium/commit/131e6ffd5ee49932acd7c8d3eec5b68648713d86) feat: add the instance_from_points node
- [7c4e11c](https://git.max-richter.dev/max/nodarium/commit/7c4e11c14da4e14818d37223ff011655dcb22a6c) feat: add the branch_from_points node
- [a3e137f](https://git.max-richter.dev/max/nodarium/commit/a3e137f593bc873565dbaf1b3bf83625ad8d2263) Merge pull request 'feat: add the random_break node' (#90) from feat/random-break-node into main
- [08f4598](https://git.max-richter.dev/max/nodarium/commit/08f4598ebef3dfaa422842aa80774bf28bd03b46) Merge remote-tracking branch 'origin/main' into feat/random-break-node
- [64c08f1](https://git.max-richter.dev/max/nodarium/commit/64c08f100917b4d926cf8bdbd9f64596b962eea4) Merge pull request 'feat: add the points_on_path node' (#89) from feat/points-on-path-node into main
- [3f6fbf5](https://git.max-richter.dev/max/nodarium/commit/3f6fbf59cef1951a93f8d637f064152bde0e9bf0) Merge pull request 'feat: roadmap tab in the changelog panel' (#88) from feat/roadmap-tab into main
- [46982e2](https://git.max-richter.dev/max/nodarium/commit/46982e29d6bec7ff33b57db58768397985f4614f) feat(ui): Tabs component, used by the changelog panel
- [c8e7039](https://git.max-richter.dev/max/nodarium/commit/c8e7039e508e30718d5797262828859c56ce3c6c) feat: add the random_break node
- [91fee27](https://git.max-richter.dev/max/nodarium/commit/91fee27afb67b183f895268e0ebcac81acb94799) feat: add the points_on_path node
- [38aa659](https://git.max-richter.dev/max/nodarium/commit/38aa659fb9d2b5fbac241ff899012eb3a28188de) feat: roadmap tab in the changelog panel
- [9cb339a](https://git.max-richter.dev/max/nodarium/commit/9cb339aa4045d9a75f15133fd68be9f366eaa577) Merge pull request 'chore: name all node inputs in snake_case' (#87) from chore/snake-case-inputs into main
- [b80daa0](https://git.max-richter.dev/max/nodarium/commit/b80daa047fe571ff50a5a6f682b93bf371544f99) fix: keep the plant shape when resolution.curve changes
- [2b85247](https://git.max-richter.dev/max/nodarium/commit/2b852470128092c7739a0736c2e40aa795117cea) chore: name all node inputs in snake_case
- [5ad72b7](https://git.max-richter.dev/max/nodarium/commit/5ad72b7dbeb33a49d4fc8e48a434dd2b4ea389d9) Merge pull request 'feat: graph versioning and migrations' (#86) from feat/graph-versioning into main
- [76e8e9b](https://git.max-richter.dev/max/nodarium/commit/76e8e9b8ee540925216527a5d262e4b62e57fd7e) Merge branch 'main' into feat/graph-versioning
- [dcb2a68](https://git.max-richter.dev/max/nodarium/commit/dcb2a68d94d777f74ccdd339da0611cfc3634ded) feat: version graphs and migrate old ones when they load
- [f21a93d](https://git.max-richter.dev/max/nodarium/commit/f21a93db7a61a656cdd14d675623cef576f7c1e1) Merge pull request 'feat: trunk kink node' (#85) from feat/trunk-kink-node into main
- [b0ad88d](https://git.max-richter.dev/max/nodarium/commit/b0ad88d41229cce0456112e742c062280a509439) feat: show points in the debug node
- [5921023](https://git.max-richter.dev/max/nodarium/commit/5921023c2519d3e90bfa2aab6c9b78b9504a7711) feat: add the trunk_kink node
- [4aaa74c](https://git.max-richter.dev/max/nodarium/commit/4aaa74ccdf06046a75192ec1170ce3143831d3e6) feat: add a points data type
- [d44fc9a](https://git.max-richter.dev/max/nodarium/commit/d44fc9a3f863d79b8f7e831394926f7bcc622b87) ci: keep builds and deploys in order across two runners
- [9a02f48](https://git.max-richter.dev/max/nodarium/commit/9a02f48872fa274ef58a2484ea982d5fadb90893) ci: run the benchmark on l.asturias only
- [6ac76bc](https://git.max-richter.dev/max/nodarium/commit/6ac76bc4bfa4b51ce490f9d81cb793ae49f7eaa6) ci: comment the branch preview link on pull requests
- [a4b8c1a](https://git.max-richter.dev/max/nodarium/commit/a4b8c1a4d61b3e7f3efd5c912e37250cab10255b) Merge pull request 'feat: let nodes have multiple outputs' (#77) from feat/multiple-node-outputs into main
- [49a3cda](https://git.max-richter.dev/max/nodarium/commit/49a3cdab5524d32f432f845ce6d4a9537deccf60) test: cover stepping the debug node through the outputs of a node
- [a38ae4e](https://git.max-richter.dev/max/nodarium/commit/a38ae4e602bc873c1ee19d78c77a53e027d3185a) ci: run checks, tests and deploy in a single job
- [dd2c54f](https://git.max-richter.dev/max/nodarium/commit/dd2c54f1cf128d2b48a8f9b17adbd2cc43e9ad66) Merge branch 'main' into feat/multiple-node-outputs
- [535c676](https://git.max-richter.dev/max/nodarium/commit/535c676af54ccee2c0ab058b96474dafceafe9ea) test: update e2e test selectors
- [57fd676](https://git.max-richter.dev/max/nodarium/commit/57fd676685746badcb267227e22f3ce65b39aea8) feat: let nodes have multiple outputs
- [98a8cba](https://git.max-richter.dev/max/nodarium/commit/98a8cba1a689edb10dd1ef482c6e088fd47a2494) feat: read typed units like 10cm or 5mm in length inputs
- [9e0162c](https://git.max-richter.dev/max/nodarium/commit/9e0162c6ce15512a255f99735d01780f0146a690) feat: retune the willow and maidenhair templates, rebuild daisy, drop banana
- [5e002fc](https://git.max-richter.dev/max/nodarium/commit/5e002fcbf0fa0ec02babfeacdda03363a3af52fc) fix: update imports for the restructured result-viewer folders
- [c14ed91](https://git.max-richter.dev/max/nodarium/commit/c14ed914b87474980f5e804504ae0a5d027a217d) perf: skip far apart segments in the avoid node
- [f698f3d](https://git.max-richter.dev/max/nodarium/commit/f698f3daf491685f932230e45133fee7cfb1dcde) perf: key pooled meshes by path, cache bounds, faster geometry hash
- [3bd5056](https://git.max-richter.dev/max/nodarium/commit/3bd50561134c7a86718dbea33786b23be1d3cb63) feat: fit 3d camera to viewport, store it per project, stop animation on input
- [2a4641c](https://git.max-richter.dev/max/nodarium/commit/2a4641c857551fb1feeeb23d41dcd75f4fde3732) feat: store node-graph camera position per project
- [78bb0ab](https://git.max-richter.dev/max/nodarium/commit/78bb0abd6934fb8bea4af2780dbfc6a2d0f9a8af) feat: add dev-only panel to push the graph into a template
- [272bb8a](https://git.max-richter.dev/max/nodarium/commit/272bb8a687186c777ed899f229fd0d38190ed785) feat: add human and arrow helpers
- [507df11](https://git.max-richter.dev/max/nodarium/commit/507df11ad62566a03312a9b03d2622126ddc52a0) Merge pull request 'Feat/path creator ids' (#68) from feat/path-creator-ids into main
- [be90673](https://git.max-richter.dev/max/nodarium/commit/be90673b26406b0c37ecd8cbc592d6ba10c9299a) fix: use a coarser near plane in camera framing
- [5c7fa17](https://git.max-richter.dev/max/nodarium/commit/5c7fa173de9554e5ef7d11c364afe4012fe54b3d) chore: implement pr feedback
- [43cfd88](https://git.max-richter.dev/max/nodarium/commit/43cfd88e687716e479a3326a10cfec729f6713b3) feat: encode creator and stable hashed ids in paths
- [fc3a2aa](https://git.max-richter.dev/max/nodarium/commit/fc3a2aaa1158138abab29d27d9efb55e97704583) Merge pull request 'feat: unit measurement sizes' (#66) from feat/unit-measurements into main
- [a4588d4](https://git.max-richter.dev/max/nodarium/commit/a4588d4f19cffeb5f2a2d4ad581fe3b2090829c9) chore: pnpm dprint fmt
- [a58bc64](https://git.max-richter.dev/max/nodarium/commit/a58bc6467c0cfd81cd18f0b0bb54d60c91b0bac3) feat: scale plant templates to their real size
- [1bd352c](https://git.max-richter.dev/max/nodarium/commit/1bd352c70d97abf2f61a904a672ce941407869f0) feat: unit measurement sizes
- [d47062d](https://git.max-richter.dev/max/nodarium/commit/d47062def8ab7863268397e138cf9ade4cf9aebf) chore: dprint fmt
- [b5118c9](https://git.max-richter.dev/max/nodarium/commit/b5118c9fa2fd33d8b9defe958905069ea3736275) fix: correctly scan tailwind tokens
- [d509c35](https://git.max-richter.dev/max/nodarium/commit/d509c3527d842db916499d62649bfa66dc7eda12) feat: keep template in url for reloads
- [779fee8](https://git.max-richter.dev/max/nodarium/commit/779fee80a8aa19093e7c584004361b35fe62d504) fix: record perf run on template page and move stats to bottom right
- [9412bfa](https://git.max-richter.dev/max/nodarium/commit/9412bfa8c612b17e29e38b30489a5014bc6f30ad) feat: show vertex and face counts on template page
- [a65e121](https://git.max-richter.dev/max/nodarium/commit/a65e1217a012342d933d93206e1c63ed57b1cffd) fix: obj and glb exporter
- [d513e9d](https://git.max-richter.dev/max/nodarium/commit/d513e9da2df4fa264968b0669068da4c81f83826) feat: update gizmo to match app style
- [3bda667](https://git.max-richter.dev/max/nodarium/commit/3bda667260d1b436d5d1ed5ccbdc32f246405249) feat: add possibility to show gizmos in 3d view
- [5f8bee4](https://git.max-richter.dev/max/nodarium/commit/5f8bee4e667a96d74b10ff4dc5be7d7ecb3a7fd3) feat: add apply_force node
- [8ed9a6b](https://git.max-richter.dev/max/nodarium/commit/8ed9a6b3ceebf02d0ffb6d24f1f67034cadf107b) feat: encode path id and path parent in path
- [a51a1e5](https://git.max-richter.dev/max/nodarium/commit/a51a1e5b7012e842f167a8aa1ead0e11b087f78a) chore: pnpm format
- [f36ab0e](https://git.max-richter.dev/max/nodarium/commit/f36ab0e12463cb8d394a975f855dd9f088c45643) chore: upgrade rust deps
- [78d158d](https://git.max-richter.dev/max/nodarium/commit/78d158de04a227ccf6f9a388652906bb52893098) chore: remove old templates
- [c2716b0](https://git.max-richter.dev/max/nodarium/commit/c2716b04ac6300f166ab9870a8669ecae4021f97) chore: allow unused label
- [8a7758b](https://git.max-richter.dev/max/nodarium/commit/8a7758bad8897a8c0e89984cf77b89dcf4e57caf) chore: update cargo toml to allow wasm imports
- [ed31592](https://git.max-richter.dev/max/nodarium/commit/ed31592ad54db166d4d3bf6a45f6c695191d034d) feat: close ends of extruded paths
- [f9acf6b](https://git.max-richter.dev/max/nodarium/commit/f9acf6bf48ca528b4b696142a327cd3ccd803296) chore: update pinned gitea action tag
- [b78f61d](https://git.max-richter.dev/max/nodarium/commit/b78f61d330632f7cb2f4f883f9970e35061410f4) chore: rename all input.json to definition.json
- [434a2d1](https://git.max-richter.dev/max/nodarium/commit/434a2d18c4033aee44ba597af363f04196c8ca97) feat: add snap node
- [2268944](https://git.max-richter.dev/max/nodarium/commit/2268944193acb2bb5fcbf57444d41d617509410f) chore: update maidenhair template
- [cf2537f](https://git.max-richter.dev/max/nodarium/commit/cf2537fe5dffd87700fb8da7bab9f95dac1d449d) feat: avoid node
- [f27da43](https://git.max-richter.dev/max/nodarium/commit/f27da437c363050ebe7a6f7ff103f14f96e8c965) feat: mute node
- [59a2d9a](https://git.max-richter.dev/max/nodarium/commit/59a2d9a5d02608c2b72f384dc33e556a5d2c3174) chore: update some templates
- [dd9771d](https://git.max-richter.dev/max/nodarium/commit/dd9771ddf1a076672b03fb9cf3eaf26c55720a2a) feat: add path join node
- [1f353fd](https://git.max-richter.dev/max/nodarium/commit/1f353fd6da6c79de24f8a456f7333475cac1e1ea) chore: update ci image deps
- [7ec7b5d](https://git.max-richter.dev/max/nodarium/commit/7ec7b5d7a0d2f67372090c60f83feab8fc5d740a) feat: /template route to inspect templates
- [787df40](https://git.max-richter.dev/max/nodarium/commit/787df403623e962850294cbbc9bc59cd0438e197) feat: add thickness input to leaves
- [40de98a](https://git.max-richter.dev/max/nodarium/commit/40de98a72c6c7671665f01f3a5d6f189a6c5193e) chore: allow to build ci manually
- [bedcd94](https://git.max-richter.dev/max/nodarium/commit/bedcd94af27cd49bb217ce4c230c849e2d7e63cf) feat: better generate resolution
- [6f21ec9](https://git.max-richter.dev/max/nodarium/commit/6f21ec9ef552945d61f10e5b7965f569660d38c4) feat: better display instances in debug view
- [4b90e3e](https://git.max-richter.dev/max/nodarium/commit/4b90e3e052e36d74c0910cefa1a38deb234b117e) fix: make header only nodes look better
- [6edeceb](https://git.max-richter.dev/max/nodarium/commit/6edeceb0ae7b8fd59cce602a113d74dd9c7ab8f9) feat: use alpha as default for curve note
- [fd5de83](https://git.max-richter.dev/max/nodarium/commit/fd5de8377e654f58354260141b95aa7bf2c218fd) fix: make more parameters work with alpha
- [48e97b7](https://git.max-richter.dev/max/nodarium/commit/48e97b78f4edb32169b97ae438ffd5da9871534a) chore: refactor a few things to modern svelte
- [7d9bec0](https://git.max-richter.dev/max/nodarium/commit/7d9bec0af7aee8fcc295ae00cb3f605f6d842cc4) chore: remove dev error icon warning
- [ccd1ec3](https://git.max-richter.dev/max/nodarium/commit/ccd1ec3d62e21235ae0b3780663a714ac9f047af) fix: correctly animate center node
- [54f5874](https://git.max-richter.dev/max/nodarium/commit/54f5874ae44a0cbcfc409e1efc20fdf733b727ca) chore: upgrade dependencies and migrate to svelte v3
- [8ca5742](https://git.max-richter.dev/max/nodarium/commit/8ca57422672c0c041ef223d32103ed6f6e231541) chore: add umami script

# v0.0.8 (2026-10-20)

## Features

- Add a random_int node for random whole numbers, so counts such as leaves per branch vary like in nature (#57).
- Add lady fern, maidenhair fern and daisy plant templates.
- Show the execution time of each node in its header in advanced mode (#23).
- Expose hidden inputs per node (#58).
- Add switchable viewer materials (Plant, Clay, Matcap, Wireframe, Normals, Faces) with rendered previews in the settings.
- Attach the debug node with Shift+right-click and draw instances in the debug view.
- Make branch resolution adaptive.
- Add an "about" section.

## Plant Templates

- Templates draw counts that vary in nature with random_int: bamboo leaves, daisy rays, lady fern pinnae and lavender flower stems
- Lady fern fronds grow from a stem node and are rotated at random around the plant
- The default plant and the leaf default shape are updated
- The plant templates doc has a new section on modelling realistic plants

## Runtime

- Name the failed node in execution errors, with its unconnected inputs and the panic location
- Highlight the failed node in the editor and log every error in a new Errors panel in advanced mode

## Fixes

- Branches sit exactly between lowestBranch and highestBranch, no more bare rachis at the tip of a frond
- Clear the viewer when a graph has no result instead of showing the previous plant (#55)
- Graphs with only the output node run and show nothing instead of failing with "Unknown input type path"

---

- [777ac3b](https://git.max-richter.dev/max/nodarium/commit/777ac3b163b6d738e7fe42e3905ed8ee7d55ad96) test: update e2e screen size so nodes are visible
- [2fa0fb2](https://git.max-richter.dev/max/nodarium/commit/2fa0fb22462b4b90d1bd6a0a9b1f4552303914d8) test(app): update e2e expected values
- [78a2d8d](https://git.max-richter.dev/max/nodarium/commit/78a2d8d3d33bead4ce9c3e2510eeef15825f895f) chore: dprint fmt
- [69be793](https://git.max-richter.dev/max/nodarium/commit/69be793c0fc43219ed9e6fb87f6abda4b0d7100d) feat: add "about" section
- [57c522b](https://git.max-richter.dev/max/nodarium/commit/57c522b18265fb0de33c1efef8e6bef973d8dc68) feat(planty): cleanup a lot of stuff
- [cbf9451](https://git.max-richter.dev/max/nodarium/commit/cbf945101792fd60d962aef24914b1a3282e46b7) chore: update default plant a bit
- [178f240](https://git.max-richter.dev/max/nodarium/commit/178f24006e8fc496b7b7863e698757adf48ac2b1) feat(runtime): name the failed node in execution errors
- [02dccdf](https://git.max-richter.dev/max/nodarium/commit/02dccdf188f2f8c4cc0751ba33e1d71cefeb3dd9) fix(nodes): give the leaf a default shape
- [a0b2cd6](https://git.max-richter.dev/max/nodarium/commit/a0b2cd6929ecf7bc6abc567c012d208769e51cbf) chore: update ci to pin gitea ssh id
- [7c06877](https://git.max-richter.dev/max/nodarium/commit/7c0687730fbcea620c416a9945b80fd39899d4d6) feat: make branch resolution adaptive
- [7ecf1f7](https://git.max-richter.dev/max/nodarium/commit/7ecf1f798a057d3bd99c57e0d39fabacbb3d426e) feat(debug): attach the debug node with shift+right-click and draw instances
- [93a0f23](https://git.max-richter.dev/max/nodarium/commit/93a0f23e79777f9ec91e534bb4003ffadf7fc5cc) feat(viewer): add switchable materials with previews in the settings
- [388ba4b](https://git.max-richter.dev/max/nodarium/commit/388ba4b436e7445652c446f0d2ca844972644905) feat: allow to expose hidden inputs per node
- [cfbe15e](https://git.max-richter.dev/max/nodarium/commit/cfbe15e7e0d28f2529cac99c5d58185c479acf65) feat(templates): grow the lady fern fronds from a stem node
- [53e72db](https://git.max-richter.dev/max/nodarium/commit/53e72db34aec7808327a0d3a353de27213504506) fix(nodes): place branches where lowestBranch and highestBranch say
- [f80364c](https://git.max-richter.dev/max/nodarium/commit/f80364cfc5ad29eae076eb28418621054d245e12) feat(templates): draw counts that vary in nature with random_int
- [a8b917b](https://git.max-richter.dev/max/nodarium/commit/a8b917be4e3c59b4959e96f7024951b789aa01c8) feat(nodes): add random_int node for random counts
- [959cf95](https://git.max-richter.dev/max/nodarium/commit/959cf95431fc97e0524fd779314523c6388ef059) feat(templates): add lady fern, maidenhair fern and daisy templates
- [d1bc01f](https://git.max-richter.dev/max/nodarium/commit/d1bc01f57e1e1ec11e596314b59cf693aaae12dd) fix(viewer): clear the old geometry when a graph has no result
- [e86f953](https://git.max-richter.dev/max/nodarium/commit/e86f95330be31ed5e1f1f195b6a855d9862269f8) feat(graph): show the execution time of each node in its header

# v0.0.7 (2026-10-18)

## Features

- Add plant templates modelled on real species, including a new Salvia officinalis (sage) template.
- Add a title bar with menus and a project browser window.
- Render project thumbnails in a worker and import/export project files.
- Add a /garden route that plants the recent projects.
- Lay out the nodes of a graph in columns (Shift+F).
- Add toasts, a zoom indicator and a confirm dialog when deleting a project.
- Render templates and projects to screenshots headlessly (`pnpm render-graph`).
- Add a language agnostic node ABI v1.

## Plant Templates

Plant templates describe real species, from the agave to the weeping willow, and are documented in `PLANT_TEMPLATES.md`.

- Agave, bamboo, fern, ornamental grass, lavender, palm, sapling, weeping willow and sage
- Sage with opposite decussate leaves, axillary leaf tufts and whorls of two-lipped flowers
- Fern and lavender grow from a crown at the golden angle, lavender from a low woody framework
- Bamboo grows as a grove of almost straight culms, the grass gets real leaf blades
- The birch sapling gets a slender trunk, hanging twigs and small rhombic leaves
- Shaped with the new alpha, curve and angle inputs
- Test graphs (Plant, Simple, Lotta Faces) are no longer listed in the project browser

## Plantarium Nodes

New nodes and inputs make it possible to vary branches and leaves along their parent.

- Alpha node for where a branch or instance sits on its parent path
- Curve node that shapes a 0-1 value
- Stem and branch thickness can follow alpha along the path
- Instances can tilt along their path with an angle input
- Phyllotactic spiral and upward angle for branches
- Noise can preserve segment lengths during displacement

## Fixes

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

## Refactors

- Evaluate float expressions lazily on a per-run context
- Use the new button component everywhere

## Chores

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
