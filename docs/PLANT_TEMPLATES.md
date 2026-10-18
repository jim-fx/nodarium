# Plant Templates

The plant templates in `app/src/lib/graph-templates` each model a real species. This file describes the structure of each plant and how its template builds it, so a template can be tuned against the real plant. Render a template with `npm run render-graph <name>` in `app/`.

Proportions are relative. The garden scales every plant to the same height, so only the ratios matter: leaf length to plant height, width to height, and so on.

## What the nodes can express

- A leaf is placed sideways from its path and faces up. `yCurve` bends it along its length: positive values bend it down, negative values bend it up. `xCurve` curls the cross-section: positive gives a channel open to the top, negative rolls the margins under (revolute, or the Λ fold of a palm leaflet).
- The size of a leaf is `leaf.size * (thickness of the path at that point + 0.01)`. Leaves therefore shrink toward the tip of their path. A thin path keeps them almost the same size because of the `+ 0.01`.
- The `angle` of a branch is measured from its parent path, not from the world. Negative angles point back toward the base of the parent.
- `gravity` in the default chain mode bends a path by at most 90°. With `mode: 0` (closed-form) it pulls every segment toward straight down without overshooting. That is how the hanging willow twigs are made.
- `branch`, `gravity`, `noise` and `instance` only change the deepest paths by default. Instancing on a path *before* branching it puts geometry only on that level, as in the grass, where the plumes sit on the flowering stems but not on the leaf blades.
- Paths taper linearly to zero, so trunks and culms are cones, not columns. Every random value comes from the same seed.

## Agave: *Agave americana* (century plant)

Family Asparagaceae, subfamily Agavoideae.

**Parts:** basal rosette, leaves (with a terminal spine and marginal teeth), the central cone of young folded leaves. The flowering stalk (scape) is not modelled.

**Structure:** The plant has no visible stem (it is *acaulescent*). The leaves sit in a dense spiral rosette, 1–2 m tall and about twice as wide. Each leaf is 1–2 m long and 15–25 cm wide (about 7:1), rigid, fleshy and lanceolate. It is narrowed above the base, broadest below the middle, and tapers to a stout dark terminal spine 2–5 cm long. The margins carry hooked teeth on small horny bumps, a few centimetres apart. The upper surface is channelled. The young inner leaves stand nearly upright and pressed together as a cone. The older outer leaves spread outward and are often bent back above the middle. After 10–30 years a single 5–9 m inflorescence grows, and the rosette dies after flowering.

**Template:** a short, thick stem. Its thickness falls off linearly, so the outer leaves are the largest. One shape is used for all leaves: narrow lanceolate, with small bumps for the teeth and a sharp tip. It feeds two `instance` nodes. 18 outer leaves curve up gently (`yCurve -1.1`). 22 inner leaves curve up strongly (`yCurve -1.45`) into the cone. Both rotate by the golden angle (2.4 rad) and are channelled (`xCurve` 0.7–0.9). The bend back above the middle is missing: a leaf can only curve one way.

## Bamboo: *Phyllostachys aurea* (golden bamboo, fishpole bamboo)

Family Poaceae, subfamily Bambusoideae.

**Parts:** rhizome, culms (woody hollow stems divided into nodes and internodes), culm sheaths, branches (two per node), branchlets, leaf sheaths, leaf blades with a short pseudopetiole.

**Structure:** A grove of upright culms, 4–8 m tall and 2–5 cm thick. They stand nearly straight and nod a little at the tip. The lower nodes are bare. Branches start at about mid-height. Each node carries a pair of branches of unequal size, on alternating sides from one node to the next. The branches are short (about 10% of the culm) and angle upward. Their thin branchlets end in fans of 2–5 leaves arranged in two ranks. The leaf blades are lanceolate, about 5–15 cm × 0.5–1.5 cm, with a long pointed tip, and hang from the branchlets.

**Template:** 11 culms grow from slightly different origins. Each is tilted and turned randomly, and gravity (elasticity 0.9) nods only the tip. Branches cover 40–98% of the culm, alternating sides (`rotation 180`) and angled 40° upward. Gravity lets their tips droop. Six leaves sit near each branch tip (60–100%) in an almost two-ranked arrangement (`rotation 2.8`), drooping (`yCurve 0.9`). The pairs of branches per node and the visible nodes are missing.

## Fern: *Nephrolepis exaltata* (Boston fern, sword fern)

Family Nephrolepidaceae.

**Parts:** short upright rhizome (crown), runners (stolons), fronds, croziers (fiddleheads, the unrolling young fronds). Each frond has a stipe (stalk), a rachis (midrib) and pinnae (leaflets) with sori (spore clusters) underneath.

**Structure:** Many fronds rise from the crown in a fountain. The inner fronds are erect and arch over, the outer ones hang. A frond is up to 1.5 m long (40–90 cm in pot forms) and *once-pinnate*. It has a short scaly stipe (about a fifth of the frond) and then 40–70 pinnae on each side, alternating, in one flat plane. Each pinna is linear-oblong, about 2–7 cm × 0.6–1.8 cm, with a crenate (scalloped) margin and a small ear (auricle) at the upper base. The pinnae stay almost the same length along the frond and only shorten near the tip, so the frond is a long strap.

**Template:** 22 fronds, each tilted 0.15–1 rad from vertical, then arched by gravity (strength 1, elasticity 0.35). The rachis is thin (0.035), so the `+ 0.01` keeps the pinnae nearly the same size along the frond. Each frond carries 110 pinnae from 13% to 99%, alternating sides (`rotation 3.14`). The bare first 13% is the stipe. The pinna shape is oblong with a scalloped margin. The auricle is missing because shapes are mirrored.

## Ornamental Grass: *Cenchrus purpurascens* (fountain grass, syn. *Pennisetum alopecuroides*)

Family Poaceae, subfamily Panicoideae.

**Parts:** tufted crown, tillers, leaf blades and sheaths, culms (flowering stems), the inflorescence: a bottlebrush-like false spike (a contracted panicle) of spikelets, each with an involucre of bristles.

**Structure:** A dense tuft (it is *caespitose*). The narrow linear leaf blades are 10–45 cm long and 3–6 mm wide, arching outward into a mound. In late summer many slender culms rise above the foliage, 60–100 cm tall, arching outward. Each ends in a cylindrical, bottlebrush-like spike 7–20 cm long. The spike is made of spikelets surrounded by fine bristles 1.5–2.5 cm long that point upward and outward. The spikes are purplish to cream and nod at the tip of the culm.

**Template:** 22 culms, tilted and turned randomly. Gravity makes the culms arch (elasticity 0.6). Then 140 bristle-shaped instances fill the top 18% of each culm, curved upward, to form the plume. After that, a `branch` node grows six leaf blades from the base of every culm (0–4% of its length, 50–85° upward, 3.2–5 long). A second gravity node and a noise node bend only the blades, so the plumes stay on the culms. The blades are thin tubes, which reads fine at garden scale.

## Lavender: *Lavandula angustifolia* (English lavender)

Family Lamiaceae.

**Parts:** woody base (the plant is a subshrub), leafy shoots, opposite leaves, square (four-angled) flowering stems, peduncles, and a spike of verticillasters. Each verticillaster is a false whorl of 6–10 flowers with bracts, a 13-veined calyx and a two-lipped purple corolla.

**Structure:** An evergreen dome, 40–60 cm high including the flowers. A woody base branches into dense leafy shoots that form a low grey-green mound. The leaves are opposite and decussate (each pair is turned 90° to the next). They are linear, about 2–5 cm × 2–5 mm (more than 10:1), with margins rolled under (*revolute*), and crowded toward the base. From the mound rise many long, bare, unbranched flowering stems, 10–35 cm above the foliage. Each ends in a slender spike 2–8 cm long of stacked flower whorls, with the lowest whorls spaced further apart.

**Template:** 48 stems spread from the base (0.15–0.8 rad) and bend only a little. The lower 40% carries 16 linear leaves at 90° steps (`rotation 1.57`, decussate). The leaves are revolute (`xCurve -0.6`) and ascending. The top 18% carries 80 small tubular florets, curved strongly upward and pressed together into a dense spike. The middle stays bare as the peduncle. True whorls and the gap below the lowest whorl are missing: instances are spaced evenly.

## Palm: *Cocos nucifera* (coconut palm)

Family Arecaceae.

**Parts:** a single trunk (stem) with ring scars of old leaves and a swollen base. The crown of pinnate fronds has a fibrous leaf-base sheath, a petiole, a rachis, and leaflets (pinnae) folded in a Λ shape (*reduplicate*). Inflorescences and fruit (coconuts) hang below the crown.

**Structure:** A slender, unbranched trunk, 20–30 m tall, often leaning and gently curved. It is topped by a crown of about 25–35 fronds in a spiral. The young fronds in the centre stand upright, the older ones spread out and the oldest hang below horizontal. A frond is 4–7 m long. It has a bare petiole about a fifth of its length, then 150–250 leaflets in two ranks, one plane on each side. The leaflets are linear-lanceolate, 50–100 cm long and only a few centimetres wide (more than 15:1). They are folded in a Λ and hang from the rachis, which gives the crown its soft, drooping look.

**Template:** a leaning trunk (noise in x and z only) with 26 fronds in the top 7% at the golden angle (137.5°). The fronds are tilted between -15° and 70°, so the crown runs from erect to drooping. Gravity arches them further. Each frond has 120 narrow leaflets from 14% (the petiole) to the tip, in two ranks (`rotation 3.14`), drooping (`yCurve 1.0`) and folded (`xCurve -0.5`). The trunk tapers to a point instead of staying the same thickness. Fruit is not modelled.

## Sapling: *Betula pendula* (silver birch), young tree

Family Betulaceae.

**Parts:** trunk with a central leader, scaffold branches, long shoots and short shoots (spurs), twigs, alternate leaves with petioles, and catkins (not modelled).

**Structure:** A young birch has a slender, straight trunk with a central leader all the way to the top (the crown is *excurrent*). The open, narrow crown is ovate to conical and you can see through it. The branches are thin and alternate in a spiral. They rise at 30–50° and arch out at the tips. The fine twigs are noticeably pendulous, which gives the species its name. The leaves are small, 3–7 cm, on 1.5–3 cm petioles. They are triangular-ovate to diamond-shaped with a broad wedge-shaped base, a long pointed tip and a doubly serrate margin. They are spread loosely along the twigs.

**Template:** a straight trunk with little noise. It carries 16 branches from 28% to 97% at 137.5°, ascending 25–50° and 2.4–3.8 long, and gravity arches their tips. Each branch has 9 twigs angled slightly down, which gravity makes hang. Each twig has 9 small leaves at the golden angle. The leaf shape has a short petiole, a broad truncate base, a sharp tip and toothed margins.

## Weeping Willow: *Salix babylonica*

Family Salicaceae.

**Parts:** short stout trunk, main limbs (scaffold), long pendulous branchlets (whips), alternate leaves, catkins (not modelled).

**Structure:** A broad, rounded tree as wide as it is tall, 10–20 m. A short, thick trunk divides low into a few large limbs. These rise steeply and arch over at their ends. Long, slender, yellowish to red-brown branchlets hang straight down from the limbs as a curtain, often down to the ground. The leaves are alternate, narrow lanceolate to linear-lanceolate, 8–15 cm × 0.5–2 cm (about 8:1), with a long tapered point and a finely serrate margin. They are spread evenly along the hanging twigs and point down along them.

**Template:** a short trunk (4.5) with 7 limbs in its upper half, rising 45–70°, arched over by gravity (strength 0.8, elasticity 0.65). Each limb carries 40 whips, 2–3.4 long. A closed-form gravity node (`mode: 0`) turns the whips straight down, whichever way they started. Each whip has 36 narrow lanceolate leaves that hang along it (`yCurve 1.4`). The curtain stops above the ground.

## Sources

- [Agave americana, NParks Flora & Fauna Web](https://www.nparks.gov.sg/florafaunaweb/flora/3/5/3506), [NZ Flora](https://www.nzflora.info/factsheet/taxon/Agave-americana.html), [iNaturalist key to A. americana varieties](https://inaturalist.laji.fi/journal/mjpapay/archives/2024/03)
- [Phyllostachys aurea, PROSEA](https://prosea-bamboos.linnaeus.naturalis.nl/linnaeus_ng/app/views/species/taxon.php?id=95696), [Guadua Bamboo](https://www.guaduabamboo.com/phyllostachys-aurea/)
- [Nephrolepis exaltata, UF/IFAS](https://plant-directory.ifas.ufl.edu/plant-directory/nephrolepis-exaltata)
- [Cenchrus purpurascens, NZ Flora](https://nzflora.info/factsheet/Taxon/Cenchrus-purpurascens.html), [AusGrass2](https://ausgrass2.myspecies.info/node/1494)
- [Lavandula angustifolia, Trees and Shrubs Online](https://www.treesandshrubsonline.org/articles/lavandula/lavandula-angustifolia/)
- [Cocos nucifera, Australian Tropical Rainforest Plants](https://apps.lucidcentral.org/rainforest/text/entities/Cocos_nucifera.htm), [NParks](https://www.nparks.gov.sg/florafaunaweb/flora/5/6/5618)
- [Betula pendula, Virginia Tech Dendrology](https://dendro.cnre.vt.edu/dendrology/carddetail.cfm?ID=15), [Trees and Shrubs Online](https://www.treesandshrubsonline.org/articles/betula/betula-pendula/)
- [Salix babylonica, Flora of North America](https://floranorthamerica.org/Salix%20babylonica), [Oregon State Landscape Plants](https://landscapeplants.oregonstate.edu/plants/salix-babylonica)
