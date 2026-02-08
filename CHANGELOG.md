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
