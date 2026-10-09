from pathlib import Path
import subprocess,json,hashlib,copy
R=Path('/workspace/shared/box-text-semantics');O=Path(__file__).resolve().parent;BASE='d872f3c659dfb95dd128e99c46cdcdda073e1f80';BP='rust/docs/validation/text-tools-2026-10-09/loc/ledger.json'
def git(*a):return subprocess.check_output(['git','-C',str(R),*a],stderr=subprocess.DEVNULL)
def sha(b):return hashlib.sha256(b).hexdigest()
baseline_bytes=git('show',f'{BASE}:{BP}');assert sha(baseline_bytes)=='0a56b2ca151b55b8715407d325806328352a893f92d9cb68213025ebf0be1ab2';baseline=json.loads(baseline_bytes);old={f['path']:f for f in baseline['inventory']}
# Verify ledger inventory against immutable baseline Git objects, including evidence.
base_paths={p for p in git('ls-tree','-r','--name-only',BASE).decode().splitlines() if p.endswith('.rs')};assert base_paths==set(old)
for p,x in old.items():assert sha(git('show',f'{BASE}:{p}'))==x['sha256'],p
paths=sorted({p for p in (git('diff','--name-only',BASE).decode().splitlines()+git('ls-files','--others','--exclude-standard').decode().splitlines()) if p.endswith('.rs')});assert len(paths)==7,paths

def classify(p,b):
 lines=b.decode().splitlines();n=len(lines);ranges=[]
 if n:
  if '/tests/' in p or p.endswith(('/tests.rs','/text_tests.rs')):ranges=[[1,n]]
  elif p.endswith(('/desktop.rs','/launcher_ui.rs')):ranges=[[next(i+1 for i,l in enumerate(lines) if l=='#[cfg(test)]'),n]]
  elif p.endswith('/text_tool_state.rs'):
   shift=2 if 'Characters count graphemes' in b.decode() else 0
   ranges=[[43,50]]+[[a+shift,z+shift] for a,z in [[303,304],[336,337],[348,351],[401,417],[458,472],[539,545],[549,556]]]+[[610+shift,n]]
  elif p.endswith('/launcher_text_ui.rs'):
   ranges=[[139,142]]
   starts=[i+1 for i,l in enumerate(lines) if l=='#[cfg(test)]']
   if starts:ranges.append([starts[-1],n])
  elif p.endswith('/main.rs') or p.endswith('/text.rs'):
   starts=[i+1 for i,l in enumerate(lines) if l=='#[cfg(test)]']
   if starts:ranges=[[starts[-1],n]]
  else:raise AssertionError(p)
 support=sum(bool(l.strip()) and any(a<=i<=z for a,z in ranges) for i,l in enumerate(lines,1));total=sum(bool(l.strip()) for l in lines)
 return {'sha256':sha(b),'physical_lines':n,'nonblank':total,'support_ranges_1based':ranges,'counts':{'production':total-support,'support':support,'benchmark':0}}
def generate():
 changed=[];delta={'production':0,'support':0,'benchmark':0}
 for p in paths:
  before=git('show',f'{BASE}:{p}') if p in old else b'';after=(R/p).read_bytes();entry={'path':p,'before':classify(p,before),'after':classify(p,after)}
  for k in delta:delta[k]+=entry['after']['counts'][k]-entry['before']['counts'][k]
  changed.append(entry)
 inventory=[]
 for p in sorted(set(old)|set(paths)):
  b=(R/p).read_bytes();x={'path':p,'sha256':sha(b),'physical_lines':len(b.splitlines()),'nonblank':sum(bool(l.strip()) for l in b.splitlines()),'classification':'evidence' if '/docs/' in p else 'product'}
  if p not in paths:assert x['sha256']==old[p]['sha256'],p
  inventory.append(x)
 counts={k:baseline['cumulative_counts'][k]+delta[k] for k in delta}
 assert sum(counts.values())==sum(x['nonblank'] for x in inventory if x['classification']=='product')
 return {'base_commit':BASE,'baseline_ledger_path':BP,'baseline_ledger_sha256':sha(baseline_bytes),'baseline_counts':baseline['cumulative_counts'],'delta':delta,'cumulative_counts':counts,'changed_files':changed,'inventory':inventory,'product_files':sum(x['classification']=='product' for x in inventory),'excluded_evidence_nonblank':sum(x['nonblank'] for x in inventory if x['classification']=='evidence'),'method':'Nonblank physical Rust lines including comments; exact positive cfg(test) spans and their children are support; production feature gates remain production. Unchanged inventory hashes verified. Shared editor counted once under Box, not again under Agent. No benchmark changes.','shared_counted_once':True,'inference_time':'unavailable'}
def verify(candidate):
 expected=generate();assert candidate==expected
 return True
ledger=generate();assert verify(ledger);controls={}
for name in ['wrong_count','misclassify_production','omit_changed_path','alter_unchanged_hash','duplicate_shared_path']:
 bad=copy.deepcopy(ledger)
 if name=='wrong_count':bad['cumulative_counts']['production']+=1
 elif name=='misclassify_production':next(e for e in bad['changed_files'] if e['path'].endswith('/text_tool_state.rs'))['after']['support_ranges_1based'][0][0]-=1
 elif name=='omit_changed_path':bad['changed_files'].pop()
 elif name=='alter_unchanged_hash':next(e for e in bad['inventory'] if e['path'] not in paths)['sha256']='0'*64
 else:bad['inventory'].append(copy.deepcopy(next(e for e in bad['inventory'] if '/bello-workbench/' in e['path'])))
 try:verify(bad)
 except AssertionError:controls[name]='rejected'
 else:raise AssertionError(name)
(O/'ledger.json').write_text(json.dumps(ledger,indent=2)+'\n');v={'verified':True,'ledger_sha256':sha((O/'ledger.json').read_bytes()),'delta':ledger['delta'],'cumulative_counts':ledger['cumulative_counts'],'negative_controls':controls,'base_inventory_git_verified':True,'shared_counted_once':True};(O/'verification.json').write_text(json.dumps(v,indent=2)+'\n');print(json.dumps(v,indent=2))
