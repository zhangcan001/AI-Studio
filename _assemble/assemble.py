"""Assemble large archive files from zlib-compressed unified patches."""
from pathlib import Path
import base64
import zlib
import subprocess


def apply_one(base_blob, dest, prefix, expect, must_contain=None):
    parts = sorted(Path(".").glob(prefix + "*"))
    if not parts:
        raise SystemExit(f"missing parts: {prefix}")
    payload = "".join(p.read_text(encoding="ascii").strip() for p in parts)
    diff = zlib.decompress(base64.b64decode(payload)).decode("utf-8")
    path = Path(dest)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(subprocess.check_output(["git", "cat-file", "blob", base_blob]))
    result = subprocess.run(
        ["patch", "-p1", "--batch", "--forward"],
        input=diff,
        text=True,
        capture_output=True,
    )
    print(result.stdout)
    if result.returncode != 0:
        print(result.stderr)
        raise SystemExit(f"patch failed: {dest}")
    digest = subprocess.check_output(["git", "hash-object", str(path)], text=True).strip()
    print(dest, len(path.read_bytes()), digest)
    if digest != expect:
        raise SystemExit(f"hash mismatch: {digest} != {expect}")
    if must_contain is not None and must_contain not in path.read_bytes():
        raise SystemExit("missing expected marker")


apply_one(
    "d47f96c3c565ecf7d26853082e99442b9a0156b3",
    "src-tauri/src/application/project_backup_service.rs",
    "_assemble/svc.patch.zlib.b64.",
    "1a7ea58dd6e1b2a10f8f3c9733d3c21bf14d1735",
    b"BACKUP_VERSION: u32 = 19",
)
apply_one(
    "bd72ba5d2e30436077993c8b0267cefdeb464179",
    "src-tauri/src/infrastructure/database/repositories/project_backup.rs",
    "_assemble/repo.patch.zlib.b64.",
    "a6925b1e08efef6162287efa5c6b7875ebd76b32",
)
print("assemble ok")
