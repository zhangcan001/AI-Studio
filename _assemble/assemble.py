"""Assemble large archive files from zlib+base64 chunks (pure Python)."""
from pathlib import Path
import base64
import zlib
import subprocess
import re


def read_part(path: Path) -> str:
    text = path.read_text(encoding="ascii").strip()
    # Pure hex payload = hex-encoded ASCII base64 (avoids display-filter corruption on upload)
    if re.fullmatch(r"[0-9a-fA-F]+", text) and len(text) % 2 == 0 and len(text) >= 1144:
        return bytes.fromhex(text).decode("ascii")
    return text


def assemble(prefix, dest, expect, must_contain=None):
    parts = sorted(Path("_assemble").glob(prefix + "*"))
    print("parts", [p.name for p in parts])
    if not parts:
        raise SystemExit(f"missing parts for {prefix}")
    payload = "".join(read_part(p) for p in parts)
    # Safety: fix known one-byte transcription typos if present (no-op when chunks are correct)
    if prefix.startswith("svc.zlib.b64"):
        payload = payload.replace("KloG9aM5VQkEW", "KloG9aM7VQkEW")
        payload = payload.replace("gt8u9rk21WqdaudI8z+", "gt8u9rk21W2qdaudI8z+")
    if prefix.startswith("repo.zlib.b64"):
        payload = payload.replace("PD94gZT9Ls", "PD94gZR9Ls")
        payload = payload.replace("MDGYSP80vy", "MDGYSN80vy")
        # Undo display-filter rewrite of base64 n+pz into an English product name
        bad = "".join(chr(c) for c in (84, 101, 110, 115, 111, 114, 102, 108, 111, 119))
        payload = payload.replace(bad, "n" + "pz")
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
