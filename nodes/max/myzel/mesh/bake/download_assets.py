"""Downloads the Poly Haven assets the substrates are baked from, see bake_substrates.py.

    python3 download_assets.py <assets>
"""

import json
import os
import sys
import urllib.request

HEADERS = {"User-Agent": "nodarium-substrates"}
MODELS = ["tree_stump_01"]
TEXTURES = ["forest_leaves_02", "forrest_ground_03", "forest_leaves_04"]


def get(url):
    return urllib.request.urlopen(urllib.request.Request(url, headers=HEADERS)).read()


def save(url, dest):
    os.makedirs(os.path.dirname(dest), exist_ok=True)
    with open(dest, "wb") as f:
        f.write(get(url))


def main():
    out = sys.argv[1]
    for asset in MODELS:
        gltf = json.loads(get(f"https://api.polyhaven.com/files/{asset}"))["gltf"]["2k"]["gltf"]
        save(gltf["url"], os.path.join(out, asset, "model.gltf"))
        for rel, f in gltf["include"].items():
            if "_diff_" in rel or rel.endswith(".bin"):
                save(f["url"], os.path.join(out, asset, rel))
    for asset in TEXTURES:
        files = json.loads(get(f"https://api.polyhaven.com/files/{asset}"))
        save(files["Diffuse"]["2k"]["jpg"]["url"], os.path.join(out, asset, "diffuse.jpg"))
        save(files["Displacement"]["2k"]["jpg"]["url"], os.path.join(out, asset, "disp.jpg"))


main()
