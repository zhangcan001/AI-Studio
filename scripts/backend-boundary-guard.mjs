import { readFileSync, readdirSync } from 'node:fs';
import { join, relative } from 'node:path';
// Lightweight lexical masking keeps offsets, including nested Rust comments and raw strings.
export function maskRust(source, maskStrings = true) {
 const out=source.split(''); const blank=(a,b)=>{for(let k=a;k<b;k++)if(out[k]!=='\n'&&out[k]!=='\r')out[k]=' ';};
 for(let i=0;i<source.length;){
  const c=source[i];if(c!=='/'&&c!=='"'&&c!=="'"&&c!=='r'&&c!=='b'){i++;continue;}
  if(source.startsWith('//',i)){const end=source.indexOf('\n',i);const j=end<0?source.length:end;blank(i,j);i=j;continue;}
  if(source.startsWith('/*',i)){let j=i+2,depth=1;while(j<source.length&&depth){if(source.startsWith('/*',j)){depth++;j+=2;}else if(source.startsWith('*/',j)){depth--;j+=2;}else j++;}blank(i,j);i=j;continue;}
  // Most source characters cannot begin literals. Avoid allocating a suffix and
  // running literal regexes at every character in full-backend CI scans.
  const raw=(source[i]==='r'||source[i]==='b')?/^(?:br|r)(#*)"/.exec(source.slice(i)):null;
  if(raw){const end='"'+raw[1];const a=i+raw[0].length;const found=source.indexOf(end,a);const j=found<0?source.length:found+end.length;if(maskStrings)blank(i,j);i=j;continue;}
  if(source[i]==='"'){let j=i+1;while(j<source.length){if(source[j]==='\\'){j+=2;continue;}if(source[j++]==='"')break;}if(maskStrings)blank(i,j);i=j;continue;}
  const char=source[i]==="'"?/^'(?:\\(?:u\{[0-9a-f]+\}|x[0-9a-f]{2}|.)|[^'\\])'/i.exec(source.slice(i)):null;if(char){if(maskStrings)blank(i,i+char[0].length);i+=char[0].length;continue;}i++;
 }
 return out.join('');
}
export function productionRust(source){
 if(!/#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]/.test(source))return source;
 const mask=maskRust(source), out=source.split('');const re=/#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]/g;let m;
 while((m=re.exec(mask))){let p=m.index+m[0].length,round=0,square=0,end=mask.length;
  for(;p<mask.length;p++){const c=mask[p];if(c==='(')round++;else if(c===')')round--;else if(c==='[')square++;else if(c===']')square--;else if(!round&&!square&&c===';'){end=p+1;break;}else if(!round&&!square&&c==='{'){let depth=1;for(p++;p<mask.length&&depth;p++){if(mask[p]==='{')depth++;if(mask[p]==='}')depth--;}end=p;break;}}
  for(let k=m.index;k<end;k++)if(out[k]!=='\n'&&out[k]!=='\r')out[k]=' ';re.lastIndex=end;
 }
 // Removed tail test modules need not be scanned repeatedly as whitespace.
 return out.join('').trimEnd();
}
export function sqlFootprint(source){
 // This is a deliberately broader superset of every token counted below.
 // A file without any such text cannot gain SQL through comment/test masking.
 if(!/sqlx|Sqlite|Pool|SELECT|INSERT|UPDATE|DELETE|PRAGMA/i.test(source))return {sqlx:0,pools:0,sql:0};
 const production=productionRust(source),code=maskRust(production),withStrings=maskRust(production,false);
 return {sqlx:(code.match(/\bsqlx\s*::/g)||[]).length,pools:(code.match(/\b(?:SqlitePool|SqliteConnection|SqliteTransaction|Pool\s*<\s*(?:sqlx\s*::\s*)?Sqlite)\b/g)||[]).length,sql:(withStrings.match(/\b(?:SELECT\s+[\s\S]{0,100}?\bFROM\b|INSERT\s+INTO\b|UPDATE\s+\w+\s+SET\b|DELETE\s+FROM\b|PRAGMA\s+\w+)/gi)||[]).length};}
export function rustFiles(root){return readdirSync(root,{withFileTypes:true}).flatMap(e=>e.isDirectory()?rustFiles(join(root,e.name)):e.name.endsWith('.rs')?[join(root,e.name)]:[]).sort();}
export function backendBoundary(root,manifest){const violations=[];let checked=0;
 for(const file of rustFiles(join(root,'src-tauri/src'))){const path=relative(root,file).replaceAll('\\','/');if(!/^src-tauri\/src\/(application|commands)\//.test(path))continue;
  const source=readFileSync(file,'utf8');const excluded=manifest.testOnlyModules.find(e=>e.path===path);
  if(excluded){const declaration=readFileSync(join(root,excluded.declaredIn),'utf8');if(!maskRust(declaration.replaceAll('\r\n','\n'),false).includes(excluded.declaration.replaceAll('\r\n','\n')))violations.push(`${path}: test-only declaration changed`);continue;}
  checked++;const footprint=sqlFootprint(source);const budget=manifest.grandfatheredSql.find(e=>e.path===path)?.budget??{sqlx:0,pools:0,sql:0};for(const key of Object.keys(footprint))if(footprint[key]>budget[key])violations.push(`${path}: ${key} ${footprint[key]} > ${budget[key]}`);
 }
 return {checked,violations};
}
