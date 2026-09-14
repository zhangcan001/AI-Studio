"""Assemble large archive files from zlib-compressed unified patches."""
from pathlib import Path
import base64
import zlib
import subprocess


def apply_one(base_blob, dest, part_glob, expect, must_contain=None):
    parts = sorted(Path("_assemble").glob(part_glob))
    print("parts", [str(p) for p in parts])
    if not parts:
        raise SystemExit(f"missing parts: {part_glob}")
    payload = "".join(p.read_text(encoding="ascii").strip() for p in parts)
    print("payload_len", len(payload))
    diff = zlib.decompress(base64.b64decode(payload)).decode("utf-8")
    print("diff_len", len(diff))
    path = Path(dest)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(subprocess.check_output(["git", "cat-file", "blob", base_blob]))
    print("base_bytes", path.stat().st_size)
    result = subprocess.run(
        ["git", "apply", "--whitespace=nowarn", "-"],
        input=diff.encode("utf-8"),
        capture_output=True,
    )
    print(result.stdout.decode("utf-8", errors="replace"))
    if result.returncode != 0:
        print(result.stderr.decode("utf-8", errors="replace"))
        raise SystemExit(f"git apply failed: {dest}")
    digest = subprocess.check_output(["git", "hash-object", str(path)], text=True).strip()
    print(dest, len(path.read_bytes()), digest)
    if digest != expect:
        raise SystemExit(f"hash mismatch: {digest} != {expect}")
    if must_contain is not None and must_contain not in path.read_bytes():
        raise SystemExit("missing expected marker")


apply_one(
    "d47f96c3c565ecf7d26853082e99442b9a0156b3",
    "src-tauri/src/application/project_backup_service.rs",
    "svc.patch.zlib.b64.*",
    "1a7ea58dd6e1b2a10f8f3c9733d3c21bf14d1735",
    b"BACKUP_VERSION: u32 = 19",
)
apply_one(
    "bd72ba5d2e30436077993c8b0267cefdeb464179",
    "src-tauri/src/infrastructure/database/repositories/project_backup.rs",
    "repo.patch.zlib.b64.*",
    "a6925b1e08efef6162287efa5c6b7875ebd76b32",
)
print("assemble ok")
