"""Reconstruct a legal pre-reset schema from immutable SQL, using owned test data.

Never downgrade an existing database or change migration markers in place.
Output must be a new TEMP directory. Source must be an isolated TEMP fixture,
not a real user data root. This is reconstructed compatibility evidence, not
an assertion that the source itself was produced by a historical binary.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import sqlite3


def inside(path, root):
    return path.resolve().is_relative_to(root.resolve())


def prepare(source, output, version):
    temporary = Path(os.environ['TEMP']).resolve()
    assert inside(source, temporary) and inside(output, temporary)
    assert source != temporary and output != temporary
    assert source.is_dir() and not output.exists()
    assert 1 <= version <= 42
    output.mkdir()
    repo = Path(__file__).resolve().parents[2]
    src = sqlite3.connect(f'{(source / "app.db").as_uri()}?mode=ro', uri=True)
    src.row_factory = sqlite3.Row
    dst = sqlite3.connect(output / 'app.db')
    dst.executescript('''CREATE TABLE _sqlx_migrations (
        version BIGINT PRIMARY KEY, description TEXT NOT NULL,
        installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
        success BOOLEAN NOT NULL, checksum BLOB NOT NULL,
        execution_time BIGINT NOT NULL);''')
    migrations = sorted((repo / 'src-tauri/migrations').glob('*.sql'))
    applied = []
    for migration in migrations:
        number = int(migration.name.split('_', 1)[0])
        if number > version:
            break
        raw = migration.read_bytes()
        dst.executescript(raw.decode('utf-8'))
        description = migration.stem.split('_', 1)[1].replace('_', ' ')
        dst.execute('INSERT INTO _sqlx_migrations '
                    '(version,description,success,checksum,execution_time) '
                    'VALUES (?,?,1,?,0)',
                    (number, description, hashlib.sha384(raw).digest()))
        dst.commit()
        applied.append(number)
    assert applied == list(range(1, version + 1))
    # Copy persisted facts, not replay write-side triggers (which would create
    # fresh pending reviews and collide with the archived review identities).
    triggers = dst.execute("SELECT name,sql FROM sqlite_master WHERE type='trigger'").fetchall()
    for name, _ in triggers:
        assert re.fullmatch(r'[a-z_]+', name)
        dst.execute(f'DROP TRIGGER {name}')
    remaps = []
    for project in src.execute('SELECT id,root_path FROM projects'):
        old = Path(project['root_path'])
        assert inside(old, temporary), 'source media must also be isolated'
        new = output / 'projects' / project['id']
        assert old.is_dir(), f'missing source project media: {project["id"]}'
        shutil.copytree(old, new)
        remaps.append((str(old), str(new)))
    for name in ['config', 'workflow_library']:
        if (source / name).is_dir():
            shutil.copytree(source / name, output / name)
    settings = output / 'config/settings.json'
    if settings.exists():
        data = json.loads(settings.read_text(encoding='utf-8-sig'))
        data['comfy']['endpoint'] = 'http://127.0.0.1:1'
        data['comfyEnvironmentProfiles'] = []
        settings.write_text(json.dumps(data), encoding='utf-8')
    src_tables = {r[0] for r in src.execute(
        "SELECT name FROM sqlite_master WHERE type='table'")}
    counts = {}
    for table, in dst.execute("SELECT name FROM sqlite_master WHERE type='table'").fetchall():
        if table.startswith('_') or table.startswith('sqlite_') or table not in src_tables:
            continue
        assert re.fullmatch(r'[a-z_]+', table)
        existing = {r['name'] for r in src.execute(f'PRAGMA table_info({table})')}
        columns = [r[1] for r in dst.execute(f'PRAGMA table_info({table})') if r[1] in existing]
        rows = src.execute(f'SELECT {",".join(columns)} FROM {table}').fetchall()
        values = []
        for row in rows:
            value = list(row)
            for i, column in enumerate(columns):
                if isinstance(value[i], str) and ('path' in column or column.endswith('root')):
                    for old, new in remaps:
                        value[i] = value[i].replace(old, new)
            values.append(value)
        dst.executemany(f'INSERT INTO {table} ({",".join(columns)}) '
                        f'VALUES ({",".join("?" for _ in columns)})', values)
        counts[table] = len(rows)
    for _, sql in triggers:
        dst.execute(sql)
    dst.commit()
    assert not dst.execute('PRAGMA foreign_key_check').fetchall()
    assert dst.execute('PRAGMA integrity_check').fetchone()[0] == 'ok'
    evidence = {'kind': 'reconstructed_pre_reset_fixture',
                'schemaVersion': version, 'rowCounts': counts,
                'sourceDatabaseSha256': hashlib.sha256((source / 'app.db').read_bytes()).hexdigest()}
    (output / 'fixture-evidence.json').write_text(json.dumps(evidence, indent=2), encoding='utf-8')
    src.close()
    dst.close()
    print(json.dumps(evidence))


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--source', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--schema-version', type=int, default=39)
    args = parser.parse_args()
    prepare(args.source.resolve(), args.output.resolve(), args.schema_version)
