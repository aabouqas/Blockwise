#!/usr/bin/env python3
"""Saves a project from your Blockwise projects as examples/<name>.bw, the
example Blockwise builds in (and people download). Run it after changing an
example in Blockwise, then rebuild.

    python3 app/export_example.py 606 brain-quiz --glyph brain --color FF7C4DFF

A .bw file is a zip holding project.json and the project's files in assets/.
"""
import argparse, json, os, sys, zipfile

def data_dir():
    if sys.platform == "darwin":
        return os.path.expanduser("~/Library/Application Support/Blockwise")
    if os.name == "nt":
        return os.path.join(os.environ["APPDATA"], "Blockwise")
    return os.path.join(os.environ.get("XDG_DATA_HOME", os.path.expanduser("~/.local/share")), "Blockwise")

ap = argparse.ArgumentParser()
ap.add_argument("project_id", type=int)
ap.add_argument("name", help="file name without .bw, e.g. brain-quiz")
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

out = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "examples", f"{a.name}.bw")

def add(z, name, data):
    # a fixed date, so exporting an unchanged project gives the same file
    info = zipfile.ZipInfo(name, date_time=(2026, 1, 1, 0, 0, 0))
    info.compress_type = zipfile.ZIP_DEFLATED
    z.writestr(info, data)

with zipfile.ZipFile(out, "w") as z:
    add(z, "project.json", json.dumps(p, indent=1, ensure_ascii=False) + "\n")
    for file in p["files"]:
        name = f'{file["name"]}.{file["ext"]}'
        with open(os.path.join(data_dir(), "files", str(a.project_id), name), "rb") as f:
            add(z, f"assets/{name}", f.read())
print(f"{p['app_name']} → examples/{a.name}.bw ({len(p['files'])} files)")
