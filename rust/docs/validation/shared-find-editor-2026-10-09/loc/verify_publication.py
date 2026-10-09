#!/usr/bin/env python3
"""Read-only Git proof binding the historical LOC composition to final source."""
import argparse,hashlib,json,subprocess
from pathlib import Path
from verify_from_git import run as verify_composition
P=Path(__file__).resolve().parent

def run(repo):
    result=verify_composition(repo)
    proof=json.loads((P/'publication-binding.json').read_text())
    ledger_bytes=(P/'ledger.json').read_bytes();d=json.loads(ledger_bytes)
    def require(x,m):
        if not x:raise ValueError(m)
    def git(*args):return subprocess.check_output(['git',*args],cwd=repo)
    def sha(b):return hashlib.sha256(b).hexdigest()
    def commit(c,expected_tree,parent=None):
        raw=git('cat-file','commit',c)
        require(hashlib.sha1(b'commit '+str(len(raw)).encode()+b'\0'+raw).hexdigest()==c,'commit hash')
        require(raw.splitlines()[0]==('tree '+expected_tree).encode(),'commit tree')
        if parent:require(('parent '+parent).encode() in raw.splitlines(),'commit parent')
    def entries(c):
        out={}
        for line in git('ls-tree','-r',c).decode().splitlines():
            meta,p=line.split('\t',1);out[p]=tuple(meta.split())
        return out
    require(sha(ledger_bytes)==proof['composition_ledger_sha256']=='afc8d365f0c41abab978a6d34499ac952aebf7dc32dc1463de6e74c5f1d534ba','historical ledger hash')
    child='e6a39466f9bf1722557cf6a350e53bb9f970bc89';parent='0e13ee6def214425865759591c37539429530d71'
    require(proof['source_commit']==child and proof['parent_commit']==parent,'final identities')
    commit(child,proof['source_tree'],parent);commit(parent,proof['parent_tree'])
    require(proof['source_tree']=='30709481594d4b63fae8ccfbba3d121c389c4da3' and proof['parent_tree']=='90943e3ccd7fe7841f175c8e74188289d3732d9e','exact source trees')
    base=entries(d['baseline_commit']);par=entries(parent);final=entries(child);editor=entries(d['editor_commit'])
    manifest=json.loads((P/'final-manifest.json').read_text())['files'];paths=set(manifest)
    delta={p for p in set(par)|set(final) if par.get(p)!=final.get(p)}
    require(delta==paths and len(paths)==5,'exact five-path source child')
    require({p:v for p,v in base.items() if p.endswith('.rs')}=={p:v for p,v in par.items() if p.endswith('.rs')},'docs parent preserves all popup Rust')
    for p in paths:
        require(par.get(p)==base.get(p),'no overlapping popup/editor path '+p)
        require(final[p]==editor[p] and sha(git('cat-file','blob',final[p][2]))==manifest[p],'five exact editor postimages '+p)
    expected=dict(base)
    for p in paths:expected[p]=editor[p]
    actual_rust={p:v for p,v in final.items() if p.endswith('.rs')}
    require(actual_rust=={p:v for p,v in expected.items() if p.endswith('.rs')},'final Rust equals audited composition')
    total=sum(sum(bool(line.strip()) for line in git('cat-file','blob',v[2]).decode().splitlines()) for v in actual_rust.values())
    require(total==d['all_tracked_rust_physical_sum_including_excluded_evidence']==proof['all_tracked_rust_nonblank']==100782,'full final physical sum')
    require(len(actual_rust)==proof['all_tracked_rust_files']==203,'full final Rust file count')
    require(proof['cumulative_counts']==d['cumulative_counts']==dict(production=58381,support=42181,benchmark=105),'final cumulative categories')
    require(proof['delta']==d['delta']==dict(production=476,support=574,benchmark=0),'final editor delta')
    require(proof['product_nonblank']==sum(d['cumulative_counts'].values())==100667 and proof['excluded_evidence_nonblank']==115,'product/evidence split')
    return dict(verified=True,source_commit=child,source_tree=proof['source_tree'],parent_commit=parent,composition_ledger_sha256=proof['composition_ledger_sha256'],delta=proof['delta'],cumulative_counts=proof['cumulative_counts'],product_nonblank=100667,product_files=201,excluded_evidence_nonblank=115,all_tracked_rust_nonblank=total,all_tracked_rust_files=203,exact_five_postimages=True,no_overlap=True,publication_source_binding='exact immutable source child; no ref publication claim')
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--repo',required=True,type=Path);a=p.parse_args();print(json.dumps(run(a.repo),indent=2))
