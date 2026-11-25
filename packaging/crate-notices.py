"""Collect the license declarations and upstream notices for this Rust build."""
import json
from pathlib import Path
import shutil
import subprocess

metadata = json.loads(subprocess.check_output([
    "cargo", "metadata", "--locked", "--offline", "--format-version", "1",
    "--manifest-path", "Cargo.toml", "--filter-platform", "x86_64-unknown-linux-gnu",
]))
destination = Path("crate-notices")
destination.mkdir(exist_ok=True)
records = []
for package in sorted(metadata["packages"], key=lambda p: (p["name"], p["version"])):
    if package["source"] is None:
        continue
    name = package["name"] + "-" + package["version"]
    records.append(f'{name}\t{package["license"] or package["license_file"]}\t{package["source"]}')
    root = Path(package["manifest_path"]).parent
    paths = [p for p in root.iterdir() if p.name.upper().startswith(("LICENSE", "LICENCE", "COPYING", "NOTICE", "COPYRIGHT"))]
    if package["license_file"]:
        paths.append(root / package["license_file"])
    for path in paths:
        if path.is_dir():
            shutil.copytree(path, destination / name / path.name, dirs_exist_ok=True)
        elif path.is_file():
            (destination / name).mkdir(exist_ok=True)
            shutil.copyfile(path, destination / name / path.name)
(destination / "packages.tsv").write_text("crate\tlicense\tsource\n" + "\n".join(records) + "\n", encoding="utf-8")
