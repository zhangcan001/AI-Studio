"""Assemble large archive files from zlib+base64 chunks (pure Python)."""
from pathlib import Path
import base64
import zlib
import subprocess


def assemble(prefix, dest, expect, must_contain=None):
    parts = sorted(Path("_assemble").glob(prefix + "*"))
    print("parts", [p.name for p in parts])
    if not parts:
        raise SystemExit(f"missing parts for {prefix}")
    payload = "".join(p.read_text(encoding="ascii").strip() for p in parts)
    # Safety: fix known one-byte transcription typo if present (no-op when chunks are correct)
    if prefix.startswith("svc.zlib.b64"):
        payload = payload.replace("KloG9aM5VQkEW", "KloG9aM7VQkEW")
        payload = payload.replace("gt8u9rk21WqdaudI8z+", "gt8u9rk21W2qdaudI8z+")
    data = zlib.decompress(base64.b64decode(payload))
    path = Path(dest)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)
    digest = subprocess.check_output(["git", "hash-object", str(path)], text=True).strip()
    print(dest, len(data), digest)
    if digest != expect:
        raise SystemExit(f"hash mismatch: {digest} != {expect}")
    if must_contain is not None and must_contain not in data:
        raise SystemExit("missing expected marker")


assemble(
    "svc.zlib.b64.",
    "src-tauri/src/application/project_backup_service.rs",
    "1a7ea58dd6e1b2a10f8f3c9733d3c21bf14d1735",
    b"BACKUP_VERSION: u32 = 19",
)
assemble(
    "repo.zlib.b64.",
    "src-tauri/src/infrastructure/database/repositories/project_backup.rs",
    "a6925b1e08efef6162287efa5c6b7875ebd76b32",
)
print("assemble ok")
