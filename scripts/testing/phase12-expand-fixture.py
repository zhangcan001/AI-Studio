import os,sqlite3,json,uuid
import datetime,subprocess
from pathlib import Path
p=Path(os.environ['TEMP'])/'ai-studio-phase12-native'/'app.db'
assert p.resolve().parent== (Path(os.environ['TEMP'])/'ai-studio-phase12-native').resolve()
assert subprocess.check_output(['powershell.exe','-NoProfile','-NonInteractive','-Command',
 '@(Get-Process ai-studio -ErrorAction SilentlyContinue).Count'],text=True).strip()=='0', 'Close owned Native first'
c=sqlite3.connect(p);c.row_factory=sqlite3.Row;c.execute('PRAGMA foreign_keys=ON')
def put(table,row):
 keys=list(row);c.execute('INSERT OR IGNORE INTO '+table+' ('+','.join(keys)+') VALUES ('+','.join('?' for _ in keys)+')',[row[k] for k in keys])
def ident(prefix,n):return prefix+'_'+str(uuid.uuid5(uuid.NAMESPACE_URL,'phase12-large/'+prefix+'/'+str(n)))
synthetic_tasks={ident('tsk',i) for i in range(50)}
task=next(dict(r) for r in c.execute("SELECT * FROM tasks WHERE project_id='prj_default' AND status='SUCCEEDED'") if r['id'] not in synthetic_tasks)
asset=dict(c.execute("SELECT * FROM assets WHERE project_id='prj_default' AND name='安全图片候选夹具' LIMIT 1").fetchone())
prompt=dict(c.execute("SELECT * FROM prompt_entries WHERE project_id='prj_default' LIMIT 1").fetchone())
version=dict(c.execute('SELECT * FROM prompt_versions WHERE prompt_id=? ORDER BY version DESC LIMIT 1',(prompt['id'],)).fetchone())
for i in range(50):
 r=task.copy();r.update(id=ident('tsk',i),generation_execution_id=None,parent_task_id=None,submission_idempotency_key=None)
 # Preserve the validated lifecycle ordering when moving synthetic tasks forward.
 shift=datetime.datetime.fromisoformat('2026-10-03T11:00:00+00:00')-datetime.datetime.fromisoformat(task['created_at'].replace('Z','+00:00'))
 for k,v in list(r.items()):
  if k.endswith('_at') and v:r[k]=(datetime.datetime.fromisoformat(v.replace('Z','+00:00'))+shift).isoformat()
 put('tasks',r)
 c.execute('UPDATE tasks SET '+','.join(k+'=?' for k in r if k!='id')+' WHERE id=?', [r[k] for k in r if k!='id']+[r['id']])
for i in range(100):
 r=asset.copy();r.update(id=ident('ast',i),name='Phase12 safe media %03d'%i,source_task_id=ident('tsk',i%50),created_at='2026-10-03T11:00:00Z',updated_at='2026-10-03T11:00:00Z');put('assets',r)
for i in range(50):
 r=prompt.copy();r.update(id=ident('prm',i),name='Phase12 prompt %03d'%i,normalized_name='phase12 prompt %03d'%i,created_at='2026-10-03T11:00:00Z',updated_at='2026-10-03T11:00:00Z');put('prompt_entries',r)
 v=version.copy();v.update(id=ident('prv',i),prompt_id=r['id'],version=1,text='Isolated performance fixture %03d'%i);put('prompt_versions',v)
c.commit()
assert not c.execute('PRAGMA foreign_key_check').fetchall()
result={'identity':'phase12-large-v1','rootKind':'owned isolated Phase11 copy expanded synthetically','tasks':c.execute("select count(*) from tasks where project_id='prj_default'").fetchone()[0], 'media':c.execute("select count(*) from assets where project_id='prj_default'").fetchone()[0], 'prompts':c.execute("select count(*) from prompt_entries where project_id='prj_default'").fetchone()[0], 'maxMigration':c.execute('select max(version) from _sqlx_migrations').fetchone()[0]}
(Path(os.environ['TEMP'])/'phase12-large-inventory.json').write_text(json.dumps(result,indent=2));print(json.dumps(result));c.close()
