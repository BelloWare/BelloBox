#!/usr/bin/env python3
"""Portable standard-library verification. No repository, compiler or network required.
Optional --repo compares exact audited source and final validation receipts there.
Reviewed support ranges are explicit, not a general Rust cfg parser.
"""
import argparse, hashlib, json
from pathlib import Path
P=Path(__file__).resolve().parent

def sha(b): return hashlib.sha256(b).hexdigest()
def objsha(typ,b):return hashlib.sha1(typ.encode()+b' '+str(len(b)).encode()+b'\0'+b).hexdigest()
def nonblank(b):return sum(bool(s.strip()) for s in b.decode().splitlines())
def require(ok, why):
    if not ok: raise ValueError(why)
def verify(root=P, repo=None):
    raw=(root/'source-manifest.json').read_bytes()
    require(sha(raw)=='6bc895531fd07553078f7a81911ba1b143db5396d325b61d653b8b74d661b427','source manifest hash')
    d=json.loads((root/'ledger.json').read_text())
    r3=json.loads((root/'source-manifest.json').read_text())
    final=json.loads((root/'final-manifest.json').read_text())
    require(d['baseline_commit']==r3['base']==final['base']=='15197a71d81c620bf60da18f0d066505e4c0dc99','baseline commit')
    c=(root/'git-objects'/d['baseline_commit']).read_bytes()
    require(objsha('commit',c)==d['baseline_commit'],'commit object hash')
    rt=c.splitlines()[0].split()[1].decode()
    require(rt==d['baseline_root_tree'],'root tree')
    blobs={}
    def walk(oid,prefix=''):
        b=(root/'git-objects'/oid).read_bytes()
        require(objsha('tree',b)==oid,'tree object '+oid)
        n=0
        while n<len(b):
            end=b.index(b'\0',n); mode,name=b[n:end].split(b' ',1); child=b[end+1:end+21].hex(); n=end+21
            path=prefix+name.decode()
            if mode==b'40000':walk(child,path+'/')
            elif mode!=b'160000':blobs[path]=child
    walk(rt)
    allfiles={e['path']:e for e in d['all_rust_files']}
    require(len(allfiles)==len(d['all_rust_files']),'duplicate inventory')
    baseline_paths={p for p in blobs if p.endswith('.rs') and p.startswith('rust/crates/')}
    changes={e['path']:e for e in d['files']}
    rust_manifest={p for p in r3['files'] if p.endswith('.rs')}
    require(set(changes)==rust_manifest and len(changes)==7,'exact seven QR Rust paths')
    require(set(allfiles)==baseline_paths|rust_manifest,'complete repository Rust scope')
    source={}; totals={'before':0,'after':0}
    for p,e in allfiles.items():
        source[p]={}
        for label in totals:
            b=(root/label/(p+'.txt')).read_bytes();source[p][label]=b
            require(sha(b)==e[label+'_sha256'],label+' SHA256 '+p)
            require(nonblank(b)==e[label+'_nonblank'],label+' nonblank '+p)
            totals[label]+=nonblank(b)
        require(e['before_present']==(p in baseline_paths),'presence '+p)
        if p in baseline_paths:
            require(objsha('blob',source[p]['before'])==blobs[p]==e['baseline_blob_sha1'],'baseline Git binding '+p)
        else:require(source[p]['before']==b'' and e['baseline_blob_sha1'] is None,'new file baseline '+p)
        if p not in changes:require(source[p]['before']==source[p]['after'],'unreviewed source change '+p)
        if repo:require((repo/p).read_bytes()==source[p]['after'],'live repository drift '+p)
    if repo:
        import subprocess
        paths=subprocess.check_output(['git','ls-files','--cached','--others','--exclude-standard','-z'],cwd=repo).decode().split('\0')
        require({p for p in paths if p.endswith('.rs') and (repo/p).exists()}==set(allfiles)|{e['path'] for e in d['excluded_standalone_rust_evidence']},'live complete Rust inventory including evidence')
    delta=dict(production=0,support=0,benchmark=0)
    for p,e in changes.items():
        require(e['after_sha256']==r3['files'][p]==final['files'][p],'frozen GIF product Rust '+p)
        for label in ('before','after'):
            b=source[p][label];require(sha(b)==e[label+'_sha256'],'changed SHA '+p)
            lines=b.decode().splitlines();spans=e['reviewed_support_ranges_1based'][label];s=set();prior=0
            for a,z in spans:
                require(1<=a<=z<=len(lines) and a>prior,'range bounds/order '+p);prior=z;s.update(range(a,z+1))
            support=sum(bool(line.strip()) and i in s for i,line in enumerate(lines,1))
            counts=dict(production=nonblank(b)-support,support=support,benchmark=0)
            require(counts==e['counts'][label],'reviewed counts '+label+' '+p)
        for k in delta:
            v=e['counts']['after'][k]-e['counts']['before'][k]
            require(v==e['delta'][k],'delta '+p);delta[k]+=v
    require(delta==d['delta'],'total delta')
    require(d['baseline_counts']==dict(production=57781,support=41069,benchmark=105),'published baseline counts')
    for k in delta:require(d['baseline_counts'][k]+delta[k]==d['cumulative_counts'][k],'cumulative '+k)
    require(totals['before']==sum(d['baseline_counts'].values())==d['physical_totals']['before'],'baseline physical sum')
    require(totals['after']==sum(d['cumulative_counts'].values())==d['physical_totals']['after'],'final physical sum')
    require(len(baseline_paths)==d['physical_totals']['before_files'],'baseline file count')
    require(len(allfiles)==d['physical_totals']['after_files'],'final file count')
    for p,e in d['baseline_evidence'].items():
        b=(root/'evidence'/p).read_bytes()
        require(sha(b)==e['sha256'] and objsha('blob',b)==e['blob_sha1']==blobs[p],'published evidence binding '+p)
    for p,h in final['files'].items():
        b=source[p]['after'] if p.endswith('.rs') else (root/'final-evidence'/p).read_bytes()
        require(sha(b)==h,'final manifest '+p)
        if repo: require(sha((repo/p).read_bytes())==h,'live final manifest '+p)
    prior=json.loads((root/'evidence/rust/docs/validation/palette-qr-2026-10-09/loc/ledger.json').read_text())
    require(prior['cumulative_counts']==d['baseline_counts'],'published baseline categories')
    prior_files={e['path']:e for e in prior['all_rust_files']}
    require(set(prior_files)==baseline_paths,'published baseline inventory')
    for path in baseline_paths:require(sha(source[path]['before'])==prior_files[path]['after_sha256'],'published baseline source '+path)
    baseline_blobs=dict(blobs)
    candidate=d['candidate_commit'];cb=(root/'git-objects'/candidate).read_bytes()
    require(objsha('commit',cb)==candidate=='4589e0280844df4a070533c7babb9d01cf430905','immutable candidate commit')
    require(('parent '+d['baseline_commit']).encode() in cb.splitlines() and cb.splitlines()[0].split()[1].decode()==d['candidate_tree'],'candidate parent/tree')
    ct=d['candidate_tree'];require(ct=='294c99a7847757d2c41217834b8dc87eab9e4765','frozen reconstructed candidate tree')
    blobs={};walk(ct)
    evidence=d['excluded_standalone_rust_evidence'];excluded={e['path'] for e in evidence}
    require(len(excluded)==len(evidence)==2 and not excluded&set(allfiles),'exact disjoint evidence inventory')
    require({p for p in blobs if p.endswith('.rs')}==set(allfiles)|excluded,'complete candidate Rust inventory')
    for path in allfiles:require(objsha('blob',source[path]['after'])==blobs[path],'candidate product Git binding '+path)
    evidence_total=0
    for e in evidence:
        path=e['path'];data=(root/'excluded-evidence'/(path+'.txt')).read_bytes()
        require(path in baseline_blobs and baseline_blobs[path]==blobs[path] and path.startswith('rust/docs/validation/'),'unchanged standalone evidence scope '+path)
        require(objsha('blob',data)==e['candidate_blob_sha1']==blobs[path] and sha(data)==e['sha256'],'excluded evidence hashes '+path)
        require(len(data)==e['bytes'] and len(data.decode().splitlines())==e['physical_lines'] and nonblank(data)==e['nonblank_physical_lines'],'excluded evidence counts '+path)
        evidence_total+=nonblank(data)
        if repo:require((repo/path).read_bytes()==data,'live evidence drift '+path)
    require(evidence_total==115,'excluded nonblank total')
    require(totals['after']+evidence_total==d['all_tracked_rust_physical_sum_including_excluded_evidence'],'all tracked Rust physical sum')
    require(len(allfiles)+len(excluded)==d['all_tracked_rust_files_including_excluded_evidence'],'all tracked Rust file count')
    frozen=(root/'frozen-rust-manifest.json').read_bytes();require(sha(frozen)=='e4cbbb6cb16a3c459b2b77ff4c86f858fba746930218709da819fb77737fd04e','v3 freeze hash')
    fm=json.loads(frozen);require(fm['base']==d['baseline_commit'] and fm['base_tree']==d['baseline_root_tree'] and fm['files']=={p:v for p,v in final['files'].items() if p.endswith('.rs')},'v3 freeze source binding')
    drift={p:dict(frozen_sha256=h,final_sha256=final['files'][p]) for p,h in r3['files'].items() if h!=final['files'][p]}
    require(drift==d['manifest_notes']['manifest_drift'],'documentation drift disclosure')
    return dict(verified=True,candidate_commit=d['candidate_commit'],excluded_evidence_nonblank=evidence_total,all_tracked_rust_physical_sum=d['all_tracked_rust_physical_sum_including_excluded_evidence'],baseline_commit=d['baseline_commit'],reviewed_changed_rust_paths=len(changes),unchanged_rust_paths=len(allfiles)-len(changes),delta=delta,cumulative_counts=d['cumulative_counts'],physical_totals=d['physical_totals'],final_manifest_paths=len(final['files']),live_repository_verified=repo is not None)
if __name__=='__main__':
    a=argparse.ArgumentParser();a.add_argument('--repo',type=Path);opts=a.parse_args()
    print(json.dumps(verify(repo=opts.repo),indent=2))
