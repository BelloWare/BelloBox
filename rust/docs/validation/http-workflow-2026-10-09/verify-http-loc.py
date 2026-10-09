from pathlib import Path
import subprocess,json,hashlib,copy,argparse
parser=argparse.ArgumentParser(description='Verify HTTP source LOC against immutable predecessor and prior Regex ledger.')
parser.add_argument('--candidate',type=Path,required=True)
parser.add_argument('--git-root',type=Path,required=True)
parser.add_argument('--output',type=Path,required=True)
args=parser.parse_args();R=args.candidate;G=args.git_root;O=args.output;O.mkdir(parents=True,exist_ok=True)
BASE='1943f5d2cddb02ca8fc637850d109c9af11ee2f1';BP='rust/docs/validation/regex-workflow-2026-10-09/loc/ledger.json'
def git(*a):return subprocess.check_output(['git','-C',str(G),*a],stderr=subprocess.DEVNULL)
def sha(b):return hashlib.sha256(b).hexdigest()
baseline_bytes=git('show',f'{BASE}:{BP}');baseline=json.loads(baseline_bytes);old={f['path']:f for f in baseline['inventory']}
base_paths={p for p in git('ls-tree','-r','--name-only',BASE).decode().splitlines() if p.endswith('.rs')};assert base_paths==set(old)
assert git('rev-parse',BASE+'^{tree}').decode().strip()=='630ecf1c33479c2cb3d17e3db29e5684048500a4'
assert baseline['cumulative_counts']==dict(production=65297,support=48391,benchmark=105)
before_bytes={p:git('show',f'{BASE}:{p}') for p in old}
for p,x in old.items():assert sha(before_bytes[p])==x['sha256'],p
current={str(p.relative_to(R)):p.read_bytes() for p in (R/'rust').rglob('*.rs') if 'target' not in p.parts}
assert set(old)<=set(current),'No baseline source may silently disappear'
paths=sorted(p for p,b in current.items() if b!=before_bytes.get(p,b''))
def positive_test_ranges(lines):
 ranges=[]
 for i,line in enumerate(lines):
  if line.strip()!='#[cfg(test)]':continue
  indent=len(line)-len(line.lstrip());j=i+1
  while lines[j].lstrip().startswith('#['):j+=1
  stripped=lines[j].strip()
  if stripped.endswith((';',',')) and '{' not in stripped:end=j
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
 return dict(sha256=sha(b),physical_lines=n,nonblank=total,support_ranges_1based=ranges,counts=dict(production=total-support,support=support,benchmark=0))
def generate():
 changed=[];delta=dict(production=0,support=0,benchmark=0)
 for p in paths:
  assert '/docs/' not in p,'Evidence changes require separate classification'
  entry=dict(path=p,before=classify(p,before_bytes.get(p,b'')),after=classify(p,current[p]))
  for k in delta:delta[k]+=entry['after']['counts'][k]-entry['before']['counts'][k]
  changed.append(entry)
 inventory=[]
 for p,b in sorted(current.items()):
  x=dict(path=p,sha256=sha(b),physical_lines=len(b.splitlines()),nonblank=sum(bool(l.strip()) for l in b.splitlines()),classification='evidence' if '/docs/' in p else 'product')
  if p not in paths:assert x['sha256']==old[p]['sha256'],p
  inventory.append(x)
 counts={k:baseline['cumulative_counts'][k]+delta[k] for k in delta}
 assert sum(counts.values())==sum(x['nonblank'] for x in inventory if x['classification']=='product')
 assert all('/bello-workbench' not in p for p in paths),'Shared delta needs separate review'
 return dict(base_commit=BASE,base_tree='630ecf1c33479c2cb3d17e3db29e5684048500a4',baseline_ledger_path=BP,baseline_ledger_sha256=sha(baseline_bytes),baseline_counts=baseline['cumulative_counts'],delta=delta,cumulative_counts=counts,changed_files=changed,inventory=inventory,method='Prior Regex nonblank physical Rust LOC method including comments. Exact positive cfg(test) spans and external test children are support; inspected changed code contains no compound cfg requiring a new classifier. Baseline inventory and blobs verified using local Git object store; candidate export bound by hashes. Shared counted once under Box; no shared delta. Provisional until final source seal.',shared_counted_once=True,inference_time='unavailable')
def verify(candidate):assert candidate==generate()
ledger=generate();verify(ledger);controls={}
for name in ['wrong_count','misclassify_production','omit_changed_path','alter_unchanged_hash','duplicate_shared_path']:
 bad=copy.deepcopy(ledger)
 if name=='wrong_count':bad['cumulative_counts']['production']+=1
 elif name=='misclassify_production':next(e for e in bad['changed_files'] if e['path'].endswith('/http_session.rs'))['after']['support_ranges_1based'][0][0]-=1
 elif name=='omit_changed_path':bad['changed_files'].pop()
 elif name=='alter_unchanged_hash':next(e for e in bad['inventory'] if e['path'] not in paths)['sha256']='0'*64
 else:bad['inventory'].append(copy.deepcopy(next(e for e in bad['inventory'] if '/bello-workbench/' in e['path'])))
 try:verify(bad)
 except AssertionError:controls[name]='rejected'
 else:raise AssertionError(name)
(O/'loc-ledger.json').write_text(json.dumps(ledger,indent=2)+'\n');v=dict(verified=True,ledger_sha256=sha((O/'loc-ledger.json').read_bytes()),delta=ledger['delta'],cumulative_counts=ledger['cumulative_counts'],negative_controls=controls,base_inventory_git_verified=True,shared_counted_once=True,provisional=True);(O/'loc-verification.json').write_text(json.dumps(v,indent=2)+'\n');print(json.dumps(v,indent=2))
