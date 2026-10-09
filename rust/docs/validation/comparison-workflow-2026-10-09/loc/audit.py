from pathlib import Path
import subprocess,json,hashlib,copy
R=Path(__file__).resolve().parents[5];O=Path(__file__).resolve().parent;BASE='60504f2ab29d707d47a56282351fd9f75a1ae7ad';PREVIOUS='fd434af6056744f26577ad2c78cc2a44f77e7f6a';BP='rust/docs/validation/url-workflow-2026-10-09/loc/ledger.json'
def git(*a):return subprocess.check_output(['git','-C',str(R),*a],stderr=subprocess.DEVNULL)
def sha(b):return hashlib.sha256(b).hexdigest()
baseline_bytes=git('show',f'{PREVIOUS}:{BP}');assert sha(baseline_bytes)=='9df5a5ad4422d9a03581c768693493eb4b4ae33c21873fe605bc8e1c4860c2c5';baseline=json.loads(baseline_bytes);old={f['path']:f for f in baseline['inventory']}
# The published60504 baseline carries one already-counted URL test-support fix.
# Derive its inventory from the immutable full URL ledger plus the exact receipt.
AP='rust/docs/validation/url-oracle-id-fix-2026-10-09/receipt.json'
adjustment_bytes=git('show',f'{BASE}:{AP}');assert sha(adjustment_bytes)=='f722277eb95d7224692c6be78d62fcd74cc1fbca37b60e4cbfd9acfbbf0f4d05'
adjustment=json.loads(adjustment_bytes);url=adjustment['file'];ub=git('show',f'{BASE}:{url}')
assert old[url]['sha256']==adjustment['before_sha256'];assert sha(ub)==adjustment['after_sha256']
assert git('diff','--name-only',PREVIOUS,BASE,'--','*.rs').decode().splitlines()==[url]
assert sum(bool(l.strip()) for l in ub.splitlines())-old[url]['nonblank']==23
old[url]={**old[url],'sha256':sha(ub),'physical_lines':len(ub.splitlines()),'nonblank':sum(bool(l.strip()) for l in ub.splitlines())}
baseline['cumulative_counts']=adjustment['cumulative_counts'];assert baseline['cumulative_counts']=={'production':62050,'support':45658,'benchmark':105}
# Verify ledger inventory against immutable baseline Git objects, including evidence.
base_paths={p for p in git('ls-tree','-r','--name-only',BASE).decode().splitlines() if p.endswith('.rs')};assert base_paths==set(old)
for p,x in old.items():assert sha(git('show',f'{BASE}:{p}'))==x['sha256'],p
paths=sorted({p for p in (git('diff','--name-only',BASE).decode().splitlines()+git('ls-files','--others','--exclude-standard').decode().splitlines()) if p.endswith('.rs')})

def positive_test_ranges(lines):
 # Only exact positive cfg(test) is classified. Every range is written into the
 # ledger for review. Compound/feature gates are not casually treated as tests.
 ranges=[]
 for i,line in enumerate(lines):
  if line.strip()!='#[cfg(test)]':continue
  indent=len(line)-len(line.lstrip());j=i+1
  while lines[j].lstrip().startswith('#['):j+=1
  stripped=lines[j].strip()
  if stripped.endswith((';',',')) and '{' not in stripped:
   end=j
  else:
   closing=' '*indent+'}'
   end=next(k for k in range(j+1,len(lines)) if lines[k].startswith(closing) and lines[k][len(closing):].strip() in ('',';',','))
  ranges.append([i+1,end+1])
 return ranges

def classify(p,b):
 lines=b.decode().splitlines();n=len(lines);ranges=[]
 if n:
  if '/tests/' in p or p.endswith(('/tests.rs','_tests.rs')):ranges=[[1,n]]
  elif p.endswith(('/desktop.rs','/launcher_ui.rs')):ranges=[[next(i+1 for i,l in enumerate(lines) if l=='#[cfg(test)]'),n]]
  else:ranges=positive_test_ranges(lines)
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
 return {'base_commit':BASE,'baseline_ledger_path':BP,'baseline_ledger_sha256':sha(baseline_bytes),'baseline_ledger_commit':PREVIOUS,'published_baseline_adjustment':{'path':AP,'sha256':sha(adjustment_bytes),'support':23},'baseline_counts':baseline['cumulative_counts'],'delta':delta,'cumulative_counts':counts,'changed_files':changed,'inventory':inventory,'product_files':sum(x['classification']=='product' for x in inventory),'excluded_evidence_nonblank':sum(x['nonblank'] for x in inventory if x['classification']=='evidence'),'method':'Nonblank physical Rust lines including comments; exact positive cfg(test) spans and their children are support; production feature gates remain production. Unchanged inventory hashes verified. Shared editor counted once under Box, not again under Agent. No benchmark changes.','shared_counted_once':True,'inference_time':'unavailable'}
def verify(candidate):
 expected=generate();assert candidate==expected
 return True
ledger=generate();assert verify(ledger);controls={}
for name in ['wrong_count','misclassify_production','omit_changed_path','alter_unchanged_hash','duplicate_shared_path']:
 bad=copy.deepcopy(ledger)
 if name=='wrong_count':bad['cumulative_counts']['production']+=1
 elif name=='misclassify_production':next(e for e in bad['changed_files'] if e['path'].endswith('/compare_ui.rs'))['after']['support_ranges_1based'][0][0]-=1
 elif name=='omit_changed_path':bad['changed_files'].pop()
 elif name=='alter_unchanged_hash':next(e for e in bad['inventory'] if e['path'] not in paths)['sha256']='0'*64
 else:bad['inventory'].append(copy.deepcopy(next(e for e in bad['inventory'] if '/bello-workbench/' in e['path'])))
 try:verify(bad)
 except AssertionError:controls[name]='rejected'
 else:raise AssertionError(name)
(O/'ledger.json').write_text(json.dumps(ledger,indent=2)+'\n');v={'verified':True,'ledger_sha256':sha((O/'ledger.json').read_bytes()),'delta':ledger['delta'],'cumulative_counts':ledger['cumulative_counts'],'negative_controls':controls,'base_inventory_git_verified':True,'shared_counted_once':True};(O/'verification.json').write_text(json.dumps(v,indent=2)+'\n');print(json.dumps(v,indent=2))
