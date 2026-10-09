#!/usr/bin/env python3
"""Rehydrate and verify the text-only audit from a supplied Git repository.
Read-only Git; temporary snapshots/objects are deleted automatically. No fetch,
ref/index/worktree changes, builds or Cargo. All three input commits must exist.
"""
import argparse,hashlib,json,shutil,subprocess,tempfile
from pathlib import Path
from verify import verify
P=Path(__file__).resolve().parent

def run(repo):
    d=json.loads((P/'ledger.json').read_text());m=json.loads((P/'final-manifest.json').read_text())
    def git(*args):return subprocess.check_output(['git',*args],cwd=repo)
    def objsha(kind,b):return hashlib.sha1(kind.encode()+b' '+str(len(b)).encode()+b'\0'+b).hexdigest()
    with tempfile.TemporaryDirectory(prefix='shared-editor-loc-') as temp:
        root=Path(temp)
        def put(path,b):
            p=root/path;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(b)
        for name in ['ledger.json','source-manifest.json','final-manifest.json','editor-manifest.json','popup-baseline-ledger.json']:
            shutil.copyfile(P/name,root/name)
        for commit in [d['baseline_commit'],d['editor_commit'],d['common_base_commit']]:
            raw=git('cat-file','commit',commit);put('git-objects/'+commit,raw)
            trees={raw.splitlines()[0].split()[1].decode()}
            for line in git('ls-tree','-r','-t',commit).decode().splitlines():
                meta,_=line.split('\t',1);_,kind,oid=meta.split()
                if kind=='tree':trees.add(oid)
            for oid in trees:put('git-objects/'+oid,git('cat-file','tree',oid))
        for e in d['all_rust_files']:
            path=e['path'];before=git('cat-file','blob',e['baseline_blob_sha1']) if e['before_present'] else b''
            after=git('show',d['editor_commit']+':'+path) if path in m['files'] else before
            put('before/'+path+'.txt',before);put('after/'+path+'.txt',after)
        for path in d['baseline_evidence']:put('evidence/'+path,git('show',d['baseline_commit']+':'+path))
        for e in d['excluded_standalone_rust_evidence']:
            put('excluded-evidence/'+e['path']+'.txt',git('cat-file','blob',e['candidate_blob_sha1']))
        updates={}
        for path in m['files']:
            blob=git('show',d['editor_commit']+':'+path);updates[path]=objsha('blob',blob)
            if not path.endswith('.rs'):put('final-evidence/'+path,blob)
        def compose(oid,changes):
            raw=(root/'git-objects'/oid).read_bytes();entries={};i=0
            while i<len(raw):
                end=raw.index(b'\0',i);mode,name=raw[i:end].split(b' ',1);child=raw[end+1:end+21].hex();i=end+21;entries[name.decode()]=(mode,child)
            grouped={}
            for path,child in changes.items():
                name,sep,rest=path.partition('/')
                if sep:grouped.setdefault(name,{})[rest]=child
                else:entries[name]=(entries.get(name,(b'100644',None))[0],child)
            for name,group in grouped.items():
                mode,child=entries[name]
                if mode!=b'40000':raise ValueError('non-directory parent '+name)
                entries[name]=(mode,compose(child,group))
            result=b''.join(mode+b' '+name.encode()+b'\0'+bytes.fromhex(child) for name,(mode,child) in sorted(entries.items(),key=lambda x:x[0]+('/' if x[1][0]==b'40000' else '')))
            oid=objsha('tree',result);put('git-objects/'+oid,result);return oid
        composed=compose(d['baseline_root_tree'],updates)
        if composed!=d['candidate_tree']:raise ValueError('composition tree mismatch')
        return verify(root=root)
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--repo',required=True,type=Path);a=p.parse_args()
    print(json.dumps(run(a.repo),indent=2))
