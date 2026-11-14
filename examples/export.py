#!/usr/bin/env python3
"""Copies a project from your Blockwise projects into examples/<folder>/,
as the example that Blockwise builds in: project.json plus its files in
assets/, and <folder>.bw, the same in one file to download and import.
Run it after changing an example in Blockwise, then rebuild.

    python3 examples/export.py 606 brain-quiz --glyph brain --color FF7C4DFF
"""
import argparse, json, os, shutil, sys, zipfile

def data_dir():
    if sys.platform == "darwin":
        return os.path.expanduser("~/Library/Application Support/Blockwise")
    if os.name == "nt":
        return os.path.join(os.environ["APPDATA"], "Blockwise")
    return os.path.join(os.environ.get("XDG_DATA_HOME", os.path.expanduser("~/.local/share")), "Blockwise")

ap = argparse.ArgumentParser()
ap.add_argument("project_id", type=int)
ap.add_argument("folder")
ap.add_argument("--glyph", help="app icon glyph, e.g. brain")
ap.add_argument("--color", help="app icon colour as AARRGGBB, e.g. FF7C4DFF")
a = ap.parse_args()

store = json.load(open(os.path.join(data_dir(), "projects.json")))
p = next((p for p in store["projects"] if p["id"] == a.project_id), None)
if p is None:
    sys.exit(f"No project {a.project_id}")
p["id"] = 0
p["modified"] = 0
p["icon_image"] = None
if a.glyph:
    p["icon_glyph"] = a.glyph
if a.color:
    p["icon_color"] = int(a.color, 16)

out = os.path.join(os.path.dirname(os.path.abspath(__file__)), a.folder)
os.makedirs(out, exist_ok=True)
with open(os.path.join(out, "project.json"), "w") as f:
    json.dump(p, f, indent=1, ensure_ascii=False)
    f.write("\n")
assets = os.path.join(out, "assets")
shutil.rmtree(assets, ignore_errors=True)
for file in p["files"]:
    name = f'{file["name"]}.{file["ext"]}'
    os.makedirs(assets, exist_ok=True)
    shutil.copy(os.path.join(data_dir(), "files", str(a.project_id), name), os.path.join(assets, name))
# the .bw file: a zip with the same layout
with zipfile.ZipFile(os.path.join(out, f"{a.folder}.bw"), "w", zipfile.ZIP_DEFLATED) as z:
    z.write(os.path.join(out, "project.json"), "project.json")
    for file in p["files"]:
        name = f'{file["name"]}.{file["ext"]}'
        z.write(os.path.join(assets, name), f"assets/{name}")
print(f"{p['app_name']} → examples/{a.folder} ({len(p['files'])} files)")
