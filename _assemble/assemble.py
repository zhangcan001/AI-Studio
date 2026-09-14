from pathlib import Path
import subprocess

manifest = {
    "svc": {
        "dest": "src-tauri/src/application/project_backup_service.rs",
        "expect": "1a7ea58dd6e1b2a10f8f3c9733d3c21bf14d1735",
    },
    "repo": {
        "dest": "src-tauri/src/infrastructure/database/repositories/project_backup.rs",
        "expect": "a6925b1e08efef6162287efa5c6b7875ebd76b32",
    },
}

for key, meta in manifest.items():
    d = Path(f"_assemble/{key}")
    parts = sorted(d.glob("part_*"))
    assert parts, f"no parts for {key}"
    text = "".join(p.read_text(encoding="utf-8") for p in parts)
    data = text.encode("utf-8")
    dest = Path(meta["dest"])
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_bytes(data)
    h = subprocess.check_output(["git", "hash-object", str(dest)], text=True).strip()
    print(key, "parts", len(parts), "bytes", len(data), "sha", h)
    assert h == meta["expect"], (h, meta["expect"])
    if key == "svc":
        assert b"BACKUP_VERSION: u32 = 19" in data
print("assemble ok")
