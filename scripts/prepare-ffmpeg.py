#!/usr/bin/env python3
"""Fetch a pinned LGPL shared FFmpeg SDK into the project's isolated tools dir."""
import hashlib
import json
from pathlib import Path
import shutil
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parents[1]
VERSION = "n8.1.3-36-ge22a4510c9"
URL = ("https://github.com/FFmpeg-Builds/FFmpeg-Builds/releases/download/"
       "autobuild-2026-11-18-12-39/ffmpeg-" + VERSION + "-win64-lgpl-shared-8.1.zip")
SHA256 = "13a795e5620236ca4c304b580f0155130bdcecd87bf5e9f66f84b59d98783668"
BASE = ROOT / ".tools/ffmpeg"
SDK = BASE / "windows-x64"
ARCHIVE = BASE / (VERSION + ".zip")

def main():
    BASE.mkdir(parents=True, exist_ok=True)
    if not ARCHIVE.exists():
        with urllib.request.urlopen(URL, timeout=60) as response, ARCHIVE.open("wb") as output:
            shutil.copyfileobj(response, output)
    if hashlib.sha256(ARCHIVE.read_bytes()).hexdigest() != SHA256:
        raise SystemExit("FFmpeg archive checksum mismatch; remove archive and retry")
    with zipfile.ZipFile(ARCHIVE) as archive:
        for member in archive.infolist():
            relative = Path(*Path(member.filename).parts[1:])
            if member.is_dir() or not relative.parts:
                continue
            target = SDK / relative
            if not target.resolve().is_relative_to(SDK.resolve()):
                raise SystemExit("Invalid archive path")
            target.parent.mkdir(parents=True, exist_ok=True)
            with archive.open(member) as src, target.open("wb") as dst:
                shutil.copyfileobj(src, dst)
    (SDK / "KOKODA-DEPENDENCY.json").write_text(json.dumps({
        "version": VERSION, "url": URL, "sha256": SHA256,
        "build_project": "https://github.com/FFmpeg-Builds/FFmpeg-Builds",
        "source": "https://github.com/FFmpeg/FFmpeg/commit/e22a4510c9",
        "license": "LGPL shared build; see LICENSE.txt"
    }, indent=2) + "\n")
    print(SDK)

if __name__ == "__main__":
    main()
