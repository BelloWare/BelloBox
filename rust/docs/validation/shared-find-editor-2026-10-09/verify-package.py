"""Read-only verifier: receipt transport, records, logs and candidate Git objects.
Optional first argument supplies local frozen checkout; never runs Cargo or peer scripts.
"""
from pathlib import Path
import base64, collections, datetime, hashlib, json, lzma, re, subprocess, sys
P=Path(__file__).resolve().parent
R=Path(sys.argv[1] if len(sys.argv)>1 else '/workspace/shared/box-find-presentation')
def sha(b): return hashlib.sha256(b).hexdigest()
def read(n): return json.loads((P/n).read_text())
def git(*a): return subprocess.check_output(['git','-C',str(R),*a])
o=read('native-comment.json'); body=o['body']
raw=base64.b64decode(''.join(re.findall(r'```text\n(.*?)\n```',body,re.S)[-1].split()),validate=True)
assert sha(raw)=='993204b951edf8ffeda44b73598ac04168cd13fbf9cb6c148777c4847046a310'
dec=lzma.decompress(raw); assert sha(dec)=='eea1a2ee65f6cd62c79d7231a818e0e04936a92bda1d11138725afb82221f9c2'
assert dec==(P/'native-evidence.json').read_bytes()
bundle=json.loads(dec); assert len(bundle['files'])==24
for n,v in bundle['files'].items():
 assert sha(v['text'].encode())==v['sha256'],n
 assert (P/'native'/n).read_bytes()==v['text'].encode(),n
v=read('native/validation.json'); ident=v['identity']; commit=ident['commit']
assert commit==bundle['candidate']=='2e72ec51bd010a96624ca7326f9a9afcb1057686'
assert git('rev-parse',commit+'^{tree}').decode().strip()==ident['tree']=='c3af9d1030f8d65df98cd9ef1b41d7759ca5ea5f'
assert git('rev-parse',commit+'^').decode().strip()==ident['parent']=='15197a71d81c620bf60da18f0d066505e4c0dc99'
manifest=read('candidate/manifest.json')['files']; expected=set(manifest)
assert set(git('diff-tree','--no-commit-id','--name-only','-r',commit).decode().splitlines())==expected
post=read('native/candidate-postimages.json'); assert {p['path'] for p in post}==expected
for x in post:
 path=x['path']; data=git('show',commit+':'+path)
 assert sha(data)==x['sha256']==manifest[path]['sha256']==ident['postimages'][path]
 assert len(data)==manifest[path]['bytes']; assert data==(R/path).read_bytes()
 assert git('rev-parse',commit+':'+path).decode().strip()==x['git_blob']
 assert git('ls-tree',commit,'--',path).decode().split()[0]==x['mode']
assert sha((P/'native/validation.json').read_bytes())=='846d33c47c50d96e6a7dcc7d4a6282892edc0bedab0b7e9789f952e8ff3ba7bd'
assert sha((P/'native/candidate-postimages.json').read_bytes())==v['postimages_sha256']==ident['postimages_sha256']
assert v['identity']==read('native/preparation.json')
steps=read('native/commands.json')['steps']; by={r['name']:r for r in v['results']}; assert len(by)==6
for step in steps:
 r=by[step['name']]; saved=read('native/'+step['name']+'-result.json')
 assert all(r[k]==val for k,val in saved.items())
 assert r['command']==step['command'] and r['exit_code']==0
 log=(P/'native'/Path(r['log']).name).read_bytes(); assert sha(log)==r['log_sha256']
 assert not re.search(r'^(warning|error)(\[|:)',log.decode(),re.M)
 assert 'future-incompat' not in log.decode() and 'future version of Rust' not in log.decode()
inventory=re.findall(r'^(\S+): test$',(P/'native/inventory.log').read_text(),re.M)
executed=re.findall(r'^test (\S+) \.\.\. (\S+)$',(P/'native/library-tests.log').read_text(),re.M)
assert len(inventory)==len(set(inventory))==len(executed)==37
assert collections.Counter(inventory)==collections.Counter(n for n,s in executed)
assert all(s=='ok' for n,s in executed)
assert inventory==by['inventory']['listed_tests'] and [list(x) for x in executed]==by['library-tests']['tests']
assert 'test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;' in (P/'native/library-tests.log').read_text()
assert by['inventory']['binaries']==by['library-tests']['binaries']
assert '--test-threads=2' in by['library-tests']['command']
assert by['ordinary-library']['started_at']>=by['clean-before-ordinary-library']['ended_at']
for name in ['inventory','ordinary-library']:
 assert 'Compiling bello-workbench-ui' in (P/'native'/f'{name}.log').read_text()
clean=read('native/clean-cache.json'); assert clean['exit_code']==0 and sha((P/'native/clean-cache.log').read_bytes())==clean['log_sha256']
assert v['source_files_and_modes_verified_before_and_after']==ident['source_files']==1925
assert v['source_manifest_sha256']==ident['source_manifest_sha256']
entries=[]
for line in git('ls-tree','-r',commit).decode().splitlines():
 meta,path=line.split('\t'); mode,kind,oid=meta.split(); assert kind=='blob'
 data=git('cat-file','blob',oid)
 entries.append({'path':path,'mode':mode,'git_blob':oid,'sha256':sha(data)})
assert len(entries)==1925
reconstructed=(json.dumps(entries,indent=2)+'\n').encode()
reconstructed_hash=sha(reconstructed)
assert reconstructed_hash==v['source_manifest_sha256'],reconstructed_hash
(P/'candidate/reconstructed-source-manifest.json').write_bytes(reconstructed)
for n,m in read('linux/log-manifest.json').items():
 data=(P/'linux'/n).read_bytes(); assert sha(data)==m['sha256'] and len(data)==m['bytes']
linux_names=re.findall(r'^test (\S+) \.\.\. ok$',(P/'linux/presentation-test-r9.log').read_text(),re.M)
ind_names=re.findall(r'^test (\S+) \.\.\. ok$',(P/'independent/independent-lib-tests.log').read_text(),re.M)
assert collections.Counter(linux_names)==collections.Counter(inventory)==collections.Counter(ind_names)
integration=read('integration/verified.json')
ichild=integration['commit']; iparent=integration['parent']
assert ichild=='e6a39466f9bf1722557cf6a350e53bb9f970bc89'
assert iparent=='0e13ee6def214425865759591c37539429530d71'
assert git('rev-parse',ichild+'^{tree}').decode().strip()==integration['tree']=='30709481594d4b63fae8ccfbba3d121c389c4da3'
assert git('rev-parse',ichild+'^').decode().strip()==iparent
assert set(git('diff-tree','--no-commit-id','--name-only','-r',ichild).decode().splitlines())==expected
for path in expected:
 assert git('show',ichild+':'+path)==git('show',commit+':'+path)
 assert git('ls-tree',ichild,'--',path)==git('ls-tree',commit,'--',path)
def tree_map(rev):
 return {line.split('\t')[1]:line.split('\t')[0] for line in git('ls-tree','-r',rev).decode().splitlines()}
before,after=tree_map(iparent),tree_map(ichild)
assert {p:v for p,v in before.items() if p not in expected}=={p:v for p,v in after.items() if p not in expected}
assert not (expected & set(git('diff','--name-only',ident['parent'],iparent).decode().splitlines()))
closure=read('integration/compilation-input-equivalence.json')
assert sha((P/'integration/compilation-input-equivalence.json').read_bytes())==integration['compilation_input_equivalence_sha256']
assert closure['original_native_candidate']==commit and closure['integrated_candidate']==ichild
for obj in closure['objects']:
 for rev in [commit,ichild]:
  assert git('rev-parse',rev+':'+obj['path']).decode().strip()==obj['git_object']
  assert git('cat-file','-t',obj['git_object']).decode().strip()==obj['type']
configs=lambda rev:{p for p in tree_map(rev) if Path(p).name in ['Cargo.toml','Cargo.lock','rust-toolchain','rust-toolchain.toml'] or (Path(p).parent.name=='.cargo' and Path(p).name in ['config','config.toml'])}
assert configs(commit)==configs(ichild)==set(closure['additional_configuration_paths'])
for path in configs(commit):assert git('show',commit+':'+path)==git('show',ichild+':'+path)
report={'verified_at':datetime.datetime.now(datetime.timezone.utc).isoformat(),'candidate':commit,'tree':ident['tree'],'parent':ident['parent'],'transport_verified':True,'embedded_records_verified':24,'five_source_postimages_match_git_and_frozen_worktree':True,'native_inventory_and_executed_names':37,'native_command_exits_verified':6,'extra_initial_clean_exit':0,'all_native_raw_log_hashes_verified':True,'candidate_source_manifest_reconstructed_from_git':reconstructed_hash,'candidate_tracked_file_count':len(entries),'native_before_after_observation':'Peer-attested by hashed finalizer receipt, not independently observed on host','native_binary_artifact_hashes':'Peer-attested; no binary transferred or executed here','linux_and_independent_test_name_sets_match':True,'no_cargo_or_peer_scripts_executed_here':True,'integrated_child':ichild,'integrated_tree':integration['tree'],'integration_parent':iparent,'integrated_five_paths_byte_blob_mode_equal':True,'all_other_parent_paths_preserved':True,'native_runtime_receipt_still_binds_standalone_candidate_only':True,'shared_crate_and_local_dependency_trees_and_cargo_configuration_equal':True}
(P/'verification.json').write_text(json.dumps(report,indent=2)+'\n'); print(json.dumps(report,indent=2))
