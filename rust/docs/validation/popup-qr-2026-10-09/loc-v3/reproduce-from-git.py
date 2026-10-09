#!/usr/bin/env python3
"""Rehydrate the audited offline inputs from immutable Git objects, then verify.
Usage: python reproduce-from-git.py --repo /path/to/BelloBox
Requires both audited commits available locally. Does not fetch or modify Git.
Temporary source snapshots are deleted on exit; none are publication artifacts.
"""
import argparse, json, shutil, subprocess, tempfile
from pathlib import Path
P=Path(__file__).resolve().parent
args=argparse.ArgumentParser();args.add_argument('--repo',type=Path,required=True);repo=args.parse_args().repo.resolve()
def git(*a):return subprocess.check_output(['git','-C',str(repo),*a])
def write(root,p,b):q=root/p;q.parent.mkdir(parents=True,exist_ok=True);q.write_bytes(b)
d=json.loads((P/'ledger.json').read_text());base=d['baseline_commit'];candidate=d['candidate_commit']
with tempfile.TemporaryDirectory(prefix='bellobox-qr-loc-') as t:
 root=Path(t)
 for n in ['ledger.json','source-manifest.json','final-manifest.json','frozen-rust-manifest.json','verify.py']:shutil.copyfile(P/n,root/n)
 seen=set()
 def obj(oid,typ):
  if oid in seen:return
  seen.add(oid);data=git('cat-file',typ,oid);write(root,'git-objects/'+oid,data)
  if typ=='commit':obj(data.splitlines()[0].split()[1].decode(),'tree')
  elif typ=='tree':
   i=0
   while i<len(data):
    z=data.index(b'\0',i);mode,name=data[i:z].split(b' ',1);child=data[z+1:z+21].hex();i=z+21
    if mode==b'40000':obj(child,'tree')
 for c in [base,candidate]:obj(c,'commit')
 for e in d['all_rust_files']:
  path=e['path']
  write(root,'before/'+path+'.txt',git('show',base+':'+path) if e['before_present'] else b'')
  write(root,'after/'+path+'.txt',git('show',candidate+':'+path))
 for path in d['baseline_evidence']:write(root,'evidence/'+path,git('show',base+':'+path))
 for path in json.loads((P/'final-manifest.json').read_text())['files']:
  if not path.endswith('.rs'):write(root,'final-evidence/'+path,git('show',candidate+':'+path))
 for e in d['excluded_standalone_rust_evidence']:write(root,'excluded-evidence/'+e['path']+'.txt',git('show',candidate+':'+e['path']))
 subprocess.run(['python3',str(root/'verify.py')],check=True)
