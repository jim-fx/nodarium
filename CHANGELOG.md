# v0.0.4 (2026-02-10)

Features

- Added shape and leaf nodes, including rotation support.
- Added high-contrast light theme and improved overall node readability.
- Enhanced UI with dots background, clearer details, and consistent node coloring.
- Improved changelog display and parsing robustness.

Fixes

- Fixed UI issues (backside rendering, missing types, linter errors).
- Improved CI handling of commit messages and changelog placement.

Chores

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
