# Plant Templates

The plant templates in `app/src/lib/graph-templates` each model a real species. This file describes the structure of each plant and how its template builds it, so a template can be tuned against the real plant. Render a template with `npm run render-graph <name>` in `app/`.

Proportions are relative. The garden scales every plant to the same height, so only the ratios matter: leaf length to plant height, width to height, and so on.

## What the nodes can express

- A leaf is placed sideways from its path and faces up. `yCurve` bends it along its length: positive values bend it down, negative values bend it up. `xCurve` curls the cross-section: positive gives a channel open to the top, negative rolls the margins under (revolute, or the Λ fold of a palm leaflet).
- The size of a leaf is `leaf.size * instance.size * (thickness of the path at that point + 0.01)`. Leaves therefore shrink toward the tip of their path, unless `alpha -> curve -> size` on the `instance` node sets another profile. Such a curve has to make up for the taper of the path: its value at a point is the wanted size divided by the thickness there.
- `alpha` is where a branch or instance sits on its parent path, from 0 at the base to 1 at the tip. `alpha -> curve` into `length`, `angle` or `rotation` of a `branch` changes the branches along the parent, into `size` of an `instance` changes the leaves along the path. The hidden `angle` of an `instance` tilts it from sticking straight out toward the tip of its path (positive) or the base (negative), as it does for a `branch`. It can't take an edge, so it is the same for all instances. A curve that drops to 0 hides the instances there, which leaves gaps between evenly spaced instances. Into `thickness` of a `stem` or `branch`, alpha goes along that path instead and the curve is its profile. `taper 0` turns off the linear taper, so the curve alone sets the profile. Everywhere else, e.g. `gravity` or `leaf`, alpha is 0.
- `branch` moves every odd branch down by `3 / resolution` of the path and every even one by `(offsetSingle - 0.5) * 6 / resolution`. `offsetSingle = step * resolution / 6`, with `step = (highestBranch - lowestBranch) / (amount - 1)`, puts branch 2k and 2k + 1 at the same height, a pair per node. The pairs move apart when `resolution.curve` changes.
- The `angle` of a branch is measured from its parent path, not from the world. Negative angles point back toward the base of the parent.
- `gravity` in the default chain mode bends a path by at most 90°. With `mode: 0` (closed-form) it pulls every segment toward straight down without overshooting. That is how the hanging willow twigs are made.
- `branch`, `gravity`, `noise` and `instance` only change the deepest paths by default. Instancing on a path _before_ branching it puts geometry only on that level, as in the grass, where the plumes sit on the flowering stems but not on the leaf blades.
- Paths taper linearly to zero unless `taper` is lowered. Every random value comes from the same seed.
- `random` draws from simplex noise, so its values gather around the middle of the range. To spread stems evenly around the plant, `branch` them from a short stem at the golden angle instead of turning them by a random angle.
- `branch` turns branch i by `i * rotation` around its parent, starting from the side (0°), 90° points down and 270° up. The rotation is read per branch, so an `alpha -> curve` into it can give every branch its own angle: the value at the alpha of branch i is the wanted angle divided by i. Branch 0 always points to the side.
- `gravity` with a negative `strength` in chain mode bends a path up instead of down, which makes shoots grow upward. It turns every segment by the same amount, so a path that is already upright tips over to the other side.
- Two `instance` nodes put leaves in opposite pairs: with `rotation 1.57`, the second one starts two steps lower, so its leaf k + 2 sits at the height of leaf k of the first and is turned 180° from it. A curve on its size hides the two extra leaves at the bottom.
- A `branch` with an almost zero `length` is a point on its parent. An `instance` on it puts its instances in a ring around that point, a true whorl.
- The shape editor sorts the points of a shape by y, so an outline has to go down from y 100 to 0 without turning back.

## Agave: _Agave americana_ (century plant)

Family Asparagaceae, subfamily Agavoideae.

**Parts:** basal rosette, leaves (with a terminal spine and marginal teeth), the central cone of young folded leaves. The flowering stalk (scape) is not modelled.

**Structure:** The plant has no visible stem (it is _acaulescent_). The leaves sit in a dense spiral rosette, 1–2 m tall and about twice as wide. Each leaf is 1–2 m long and 15–25 cm wide (about 7:1), rigid, fleshy and lanceolate. It is narrowed above the base, broadest below the middle, and tapers to a stout dark terminal spine 2–5 cm long. The margins carry hooked teeth on small horny bumps, a few centimetres apart. The upper surface is channelled. The young inner leaves stand nearly upright and pressed together as a cone. The older outer leaves spread outward and are often bent back above the middle. After 10–30 years a single 5–9 m inflorescence grows, and the rosette dies after flowering.

**Template:** a short, thick stem. One shape is used for all leaves: narrow lanceolate, with small bumps for the teeth and a sharp tip. The outline is narrower than 7:1, because the channelled cross-section adds width at the margins. It feeds two `instance` nodes. 18 outer leaves curve up gently (`yCurve -1.1`). 22 inner leaves curve past vertical (`yCurve -1.8`), so their tips lean together into the cone. Both rotate by the golden angle (2.4 rad) and are channelled (`xCurve` 0.7–0.9). One `alpha -> curve` sets the size of both, so the young leaves of the cone stay long instead of shrinking with the stem. The bend back above the middle is missing: a leaf can only curve one way.

## Bamboo: _Phyllostachys aurea_ (golden bamboo, fishpole bamboo)

Family Poaceae, subfamily Bambusoideae.

**Parts:** rhizome, culms (woody hollow stems divided into nodes and internodes), culm sheaths, branches (two per node), branchlets, leaf sheaths, leaf blades with a short pseudopetiole.

**Structure:** A grove of upright culms, 4–8 m tall and 2–5 cm thick. They stand nearly straight and nod a little at the tip. The lower nodes are bare. Branches start at about mid-height. Each node carries a pair of branches of unequal size, on alternating sides from one node to the next. The branches are short (about 10% of the culm) and angle upward. Their thin branchlets end in fans of 2–5 leaves arranged in two ranks. The leaf blades are lanceolate, about 5–15 cm × 0.5–1.5 cm, with a long pointed tip, and hang from the branchlets.

**Template:** 11 culms come up apart from each other, as in a grove of a running bamboo. They stand almost straight (0.02–0.12 rad) and lean to random sides, the random angle spans four turns so the sides spread evenly. Gravity (elasticity 0.9) nods only the tip. The culms are columns (`taper 0`) that only thin out in their top 15%. 28 branches from about 40% of the culm up stand in pairs: `offsetSingle` puts both branches of a pair at the same node, `rotation 90` turns them 90° apart and every next pair to the other side. Their length is random and falls off toward the top of the culm, and their angle rises from 35° to 60°. Gravity lets their tips droop. Four leaves sit near each branch tip (60–100%) in an almost two-ranked arrangement (`rotation 2.8`). They point 50° toward the branch tip (`angle 50`) and droop a little (`yCurve 0.6`), so they form outward sprays. The two branches of a pair differ in size only by chance, and the visible nodes are missing.

## Fern: _Nephrolepis exaltata_ (Boston fern, sword fern)

Family Nephrolepidaceae.

**Parts:** short upright rhizome (crown), runners (stolons), fronds, croziers (fiddleheads, the unrolling young fronds). Each frond has a stipe (stalk), a rachis (midrib) and pinnae (leaflets) with sori (spore clusters) underneath.

**Structure:** Many fronds rise from the crown in a fountain. The inner fronds are erect and arch over, the outer ones hang. A frond is up to 1.5 m long (40–90 cm in pot forms) and _once-pinnate_. It has a short scaly stipe (about a fifth of the frond) and then 40–70 pinnae on each side, alternating, in one flat plane. Each pinna is linear-oblong, about 2–7 cm × 0.6–1.8 cm, with a crenate (scalloped) margin and a small ear (auricle) at the upper base. The pinnae stay almost the same length along the frond and only shorten near the tip, so the frond is a long strap.

**Template:** 22 fronds grow from a short crown at the golden angle, so they spread evenly around the plant. An `alpha -> curve` tilts them by age: the outer, older fronds low on the crown spread out (33° above horizontal), the young ones in the middle stand upright (81°), ±6° at random. Gravity arches them (strength 1, elasticity 0.35). Each frond carries 110 pinnae from 13% to 99%, alternating sides (`rotation 3.14`) and slightly ascending (`angle 15`). The bare first 13% is the stipe. An `alpha -> curve` on their size keeps the pinnae the same length along most of the frond, slightly shorter at its base and short at its tip, so the frond is a strap. The pinna shape is oblong with a scalloped margin. The auricle is missing because shapes are mirrored.

## Ornamental Grass: _Cenchrus purpurascens_ (fountain grass, syn. _Pennisetum alopecuroides_)

Family Poaceae, subfamily Panicoideae.

**Parts:** tufted crown, tillers, leaf blades and sheaths, culms (flowering stems), the inflorescence: a bottlebrush-like false spike (a contracted panicle) of spikelets, each with an involucre of bristles.

**Structure:** A dense tuft (it is _caespitose_). The narrow linear leaf blades are 10–45 cm long and 3–6 mm wide, arching outward into a mound. In late summer many slender culms rise above the foliage, 60–100 cm tall, arching outward. Each ends in a cylindrical, bottlebrush-like spike 7–20 cm long. The spike is made of spikelets surrounded by fine bristles 1.5–2.5 cm long that point upward and outward. The spikes are purplish to cream and nod at the tip of the culm.

**Template:** 22 culms, tilted and turned randomly. Gravity makes the culms arch (elasticity 0.6). Then 140 bristle-shaped instances fill the top 18% of each culm, pointing upward and outward (`angle 30`, `yCurve -0.6`), to form the plume. An `alpha -> curve` on their size makes the plume a cylinder rounded at both ends. After that, two `instance` nodes put eight leaf blades on the base of every culm (0–4% of its length). The blade is a linear shape with a long fine tip and enough points to bend smoothly. Four start 72° up and arch over a little (`yCurve 0.9`), four start 62° up and arch further (`yCurve 1.4`), so they form the mound. `xCurve` stays 0: it would turn the midrib bulge of the leaf sideways and make the thin blades wide. The culms lean to random sides over four turns, so they spread evenly.

## Lavender: _Lavandula angustifolia_ (English lavender)

Family Lamiaceae.

**Parts:** woody base (the plant is a subshrub), leafy shoots, opposite leaves, square (four-angled) flowering stems, peduncles, and a spike of verticillasters. Each verticillaster is a false whorl of 6–10 flowers with bracts, a 13-veined calyx and a two-lipped purple corolla.

**Structure:** An evergreen dome, 40–60 cm high including the flowers. A woody base branches into dense leafy shoots that form a low grey-green mound. The leaves are opposite and decussate (each pair is turned 90° to the next). They are linear, about 2–5 cm × 2–5 mm (more than 10:1), with margins rolled under (_revolute_), and crowded toward the base. From the mound rise many long, bare, unbranched flowering stems, 10–35 cm above the foliage. Each ends in a slender spike 2–8 cm long of stacked flower whorls, with the lowest whorls spaced further apart.

**Template:** A short woody crown branches into 12 woody stems at the golden angle, 15–45° above the ground. Each carries 4 flower stems in its outer 60%, turned to alternating sides of up by an `alpha -> curve` into the branch `rotation`. A weak negative gravity lets them rise, so the low ones turn up and the upright ones lean out a little: the spikes end on a dome. The lower 40% carries 16 linear leaves at 90° steps (`rotation 1.57`, decussate). The leaves are revolute (`xCurve -0.6`) and ascending along the shoot (`angle 40`). The top 26% carries 120 small tubular florets, angled up along the stem (`angle 45`) and curved upward, so they press together into a compact spike. Their size follows an `alpha -> curve` that drops to 0 twice: two separate whorls sit below the dense spike, which thins out at its tip. The middle stays bare as the peduncle. The whorls are bands of florets in a spiral, not true rings.

## Palm: _Cocos nucifera_ (coconut palm)

Family Arecaceae.

**Parts:** a single trunk (stem) with ring scars of old leaves and a swollen base. The crown of pinnate fronds has a fibrous leaf-base sheath, a petiole, a rachis, and leaflets (pinnae) folded in a Λ shape (_reduplicate_). Inflorescences and fruit (coconuts) hang below the crown.

**Structure:** A slender, unbranched trunk, 20–30 m tall, often leaning and gently curved. It is topped by a crown of about 25–35 fronds in a spiral. The young fronds in the centre stand upright, the older ones spread out and the oldest hang below horizontal. A frond is 4–7 m long. It has a bare petiole about a fifth of its length, then 150–250 leaflets in two ranks, one plane on each side. The leaflets are linear-lanceolate, 50–100 cm long and only a few centimetres wide (more than 15:1). They are folded in a Λ and hang from the rachis, which gives the crown its soft, drooping look.

**Template:** a leaning trunk (noise in x and z only). Its thickness is an `alpha -> curve` with `taper 0`: a swollen base, then a slender column that only thins out inside the crown. 26 fronds sit in the top of the trunk at the golden angle (137.5°). An `alpha -> curve` tilts them by age: the lowest, oldest fronds start 20° below horizontal, the highest, youngest ones 75° above. Gravity arches them further. The youngest fronds are also shorter. Each frond has 120 narrow leaflets from 14% (the petiole) to the tip, in two ranks (`rotation 3.14`). They meet the rachis at an acute angle (`angle 30`, toward the frond tip), droop (`yCurve 1.0`) and are folded (`xCurve -0.5`). An `alpha -> curve` on their size makes them short at the base of the frond, longest past the middle and short at the tip. Fruit is not modelled.

## Sage: _Salvia officinalis_ (common sage, garden sage)

Family Lamiaceae.

**Parts:** woody base (the plant is an evergreen subshrub), square young stems, opposite leaves with long petioles, small axillary leaf tufts, and terminal racemes of verticillasters. Each verticillaster is a false whorl of 4–12 flowers with a bell-shaped, two-lipped calyx and a two-lipped violet corolla.

**Structure:** A rounded mound 30–60 (80) cm tall and at least as wide. Woody stems spread from the base, lie on the ground at first and then rise (they are _decumbent_). Their leafy shoots grow upward. The leaves are opposite and decussate, grey-green, oblong-elliptic, 2–8 × 0.8–2.5 cm (about 3:1), with a wedge-shaped to rounded base, a blunt tip and a finely crenulate margin. The upper surface is wrinkled (_rugose_), the lower one white with hairs. The petiole is up to 3 cm. The leaves keep about the same size along the shoot, get smaller toward its tip, and often carry tufts of small leaves in their axils. In early summer the shoots end in erect flowering stems with a few pairs of smaller leaves and a raceme 4–18 cm long. The verticillasters are spaced well apart (_remote_). The calyx is 8–14 mm, often purplish. The corolla is 15–30 mm, with a tube that is mostly inside the calyx, a straight to slightly sickle-shaped hooded upper lip (_galea_) and a broad, three-lobed lower lip that bends down.

**Template:** a short woody crown with 9 woody stems at the golden angle, spread evenly around the plant. They start 8–42° above the ground, a negative gravity bends them up and noise makes them gnarled. Each carries 7 shoots from 35% up. An `alpha -> curve` into the branch `rotation` turns them to alternating sides of up (225°, 315°, ...), a weak negative gravity straightens them further. Every shoot has 8 opposite, decussate leaf pairs (two `instance` nodes, see above) and 8 smaller pairs turned 90° from them as the axillary tufts. The leaf shape has a petiole a third as long as the blade, an oblong blade 2.8 times as long as wide, and a crenate margin. Each shoot ends in a flowering stem (a `branch` at its tip, `angle 90`) with 3 pairs of small, almost sessile leaves and 7 verticillasters in its upper half. Every verticillaster is a branch of length 0.002 with 6 flowers in a ring. A flower is three instances on the same point: a calyx rolled into a toothed bell (`xCurve 2.2`), the upper lip (`xCurve -1.6`, a hood) and the lower lip, which has no width inside the calyx and bends down past its mouth (`yCurve 1.5`). Random sizes make some flowers buds. Bracts, the square stems and the hairs are missing.

## Sapling: _Betula pendula_ (silver birch), young tree

Family Betulaceae.

**Parts:** trunk with a central leader, scaffold branches, long shoots and short shoots (spurs), twigs, alternate leaves with petioles, and catkins (not modelled).

**Structure:** A young birch has a slender, straight trunk with a central leader all the way to the top (the crown is _excurrent_). The open, narrow crown is ovate to conical and you can see through it. The branches are thin and alternate in a spiral. They rise at 30–50° and arch out at the tips. The fine twigs are noticeably pendulous, which gives the species its name. The leaves are small, 3–7 cm, on 1.5–3 cm petioles. They are triangular-ovate to diamond-shaped with a broad wedge-shaped base, a long pointed tip and a doubly serrate margin. They are spread loosely along the twigs.

**Template:** a straight trunk with little noise. It carries 16 branches from 28% to 97% at 137.5°, and gravity arches their tips. `alpha -> curve` shapes the crown: the branches are longest just below the middle and get shorter to the top, and they rise at 25° low on the trunk and 55° at the top, ±7° at random. The trunk is slender (0.11 thick for a height of 9). Each branch has 12 twigs angled slightly down, which a strong gravity (0.9) makes hang. The twigs get shorter toward the end of the branch. Each twig has 14 small leaves at the golden angle, tilted 40° toward the twig tip, so they hang along the pendulous twigs. The leaf shape has a petiole a third as long as the leaf and a rhombic-ovate blade, broadest a third of the way up, with a broad wedge-shaped base, a long pointed tip and a doubly serrate margin.

## Weeping Willow: _Salix babylonica_

Family Salicaceae.

**Parts:** short stout trunk, main limbs (scaffold), long pendulous branchlets (whips), alternate leaves, catkins (not modelled).

**Structure:** A broad, rounded tree as wide as it is tall, 10–20 m. A short, thick trunk divides low into a few large limbs. These rise steeply and arch over at their ends. Long, slender, yellowish to red-brown branchlets hang straight down from the limbs as a curtain, often down to the ground. The leaves are alternate, narrow lanceolate to linear-lanceolate, 8–15 cm × 0.5–2 cm (about 8:1), with a long tapered point and a finely serrate margin. They are spread evenly along the hanging twigs and point down along them.

**Template:** a short trunk (4.5) with a flared base that keeps its thickness up to the limbs (`taper 0` and an `alpha -> curve`), so the 7 limbs in its upper half are stout. They rise 45–70° and are arched over by gravity (strength 0.8, elasticity 0.65). Each limb carries 40 whips. An `alpha -> curve` makes them longer toward the outer end of the limb, so the curtain hangs lowest around the edge of the crown. A closed-form gravity node (`mode: 0`) turns the whips straight down, whichever way they started. Each whip has 36 narrow lanceolate leaves that point down along it (`angle 55`, `yCurve 0.4`) and get smaller toward its tip. The curtain stops above the ground.

## Sources

- [Agave americana, NParks Flora & Fauna Web](https://www.nparks.gov.sg/florafaunaweb/flora/3/5/3506), [NZ Flora](https://www.nzflora.info/factsheet/taxon/Agave-americana.html), [iNaturalist key to A. americana varieties](https://inaturalist.laji.fi/journal/mjpapay/archives/2024/03)
- [Phyllostachys aurea, PROSEA](https://prosea-bamboos.linnaeus.naturalis.nl/linnaeus_ng/app/views/species/taxon.php?id=95696), [Guadua Bamboo](https://www.guaduabamboo.com/phyllostachys-aurea/)
- [Nephrolepis exaltata, UF/IFAS](https://plant-directory.ifas.ufl.edu/plant-directory/nephrolepis-exaltata)
- [Cenchrus purpurascens, NZ Flora](https://nzflora.info/factsheet/Taxon/Cenchrus-purpurascens.html), [AusGrass2](https://ausgrass2.myspecies.info/node/1494)
- [Lavandula angustifolia, Trees and Shrubs Online](https://www.treesandshrubsonline.org/articles/lavandula/lavandula-angustifolia/)
- [Cocos nucifera, Australian Tropical Rainforest Plants](https://apps.lucidcentral.org/rainforest/text/entities/Cocos_nucifera.htm), [NParks](https://www.nparks.gov.sg/florafaunaweb/flora/5/6/5618)
- [Salvia officinalis, Trees and Shrubs Online](https://www.treesandshrubsonline.org/articles/salvia/salvia-officinalis/), [Go Botany](https://gobotany.nativeplanttrust.org/species/salvia/officinalis/), [Horticultural Flora of South-eastern Australia](https://hortflora.rbg.vic.gov.au/taxon/ada1d1da-5340-11e7-b82b-005056b0018f), [Giardino della Flora Appenninica](https://giardinocapracotta.unimol.it/en/?p=3277), [Flora of Tropical East Africa via POWO](https://powo.science.kew.org/taxon/urn:lsid:ipni.org:names:456833-1/general-information)
- [Betula pendula, Virginia Tech Dendrology](https://dendro.cnre.vt.edu/dendrology/carddetail.cfm?ID=15), [Trees and Shrubs Online](https://www.treesandshrubsonline.org/articles/betula/betula-pendula/)
- [Salix babylonica, Flora of North America](https://floranorthamerica.org/Salix%20babylonica), [Oregon State Landscape Plants](https://landscapeplants.oregonstate.edu/plants/salix-babylonica)
