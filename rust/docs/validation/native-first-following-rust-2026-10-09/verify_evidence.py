from pathlib import Path
from collections import Counter
import datetime,hashlib,json,os,re,subprocess
E=Path(__file__).resolve().parent
repo=E.parents[3]
setup=json.loads((E/'setup.json').read_text())
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
owned={
 'rust/crates/bello-platform/src/movie.rs',
 'rust/crates/bello-platform/src/movie/macos/fixtures.rs',
 'rust/crates/bello-platform/src/movie/macos/tests.rs',
 'rust/crates/bellobox-app/src/gif_converter/native_host_tests.rs',
 'rust/crates/bello-platform/src/movie/macos/fixture_media/positive-composition.mov',
 'rust/crates/bello-platform/src/movie/macos/fixture_media/zero-origin.mov',
}
base=json.loads((E/'base-manifest.json').read_text());unchanged=0
for row in base:
 p=repo/row['path'];mode='120000' if p.is_symlink() else ('100755' if p.stat().st_mode & 0o111 else '100644')
 assert mode==row['mode'],row['path']
 if row['path'] not in owned:
  assert sha(p)==row['sha256'],row['path'];unchanged+=1
assert len(base)==1533 and unchanged==1529
before=json.loads((E/'build-source-before.json').read_text())
for row in before:
 p=repo/row['path'];assert sha(p)==row['sha256'],row['path']
 assert ('100755' if p.stat().st_mode & 0o111 else '100644')==row['mode'],row['path']
assert len(before)==1535
path='rust/crates/bello-platform/src/movie.rs'
original=subprocess.check_output(['git','show',setup['base']+':'+path],cwd=repo).decode()
current=(repo/path).read_text();marker='/// Nondefault cross-crate CI seam:'
assert original.split(marker)[0]==current.split(marker)[0]
assert 'pub const NATIVE_MOVIE_READER_IMPLEMENTED: bool = false;' in current
for record in json.loads((E/'fixture-provenance.json').read_text()).values():
 assert sha(repo/record['destination'])==record['sha256']==sha(repo/record['source_at_base'])
results=[]
for name,count in [('platform-movie',24),('core-gif',49),('app-native-host',3)]:
 r=json.loads((E/(name+'-result.json')).read_text());log=E/(name+'.log');text=log.read_text()
 assert r['exit_code']==0 and r['counts'][-1]['passed']==count and r['counts'][-1]['failed']==0 and r['counts'][-1]['ignored']==0
 assert sha(log)==r['log_sha256']
 blocks=re.findall(r'^test (\S+) \.\.\. (.*?)(?=^test \S+ \.\.\. |^test result:)',text,re.M|re.S)
 assert len(blocks)==count and len(set(x[0] for x in blocks))==count
 assert all(body.strip().splitlines()[-1]=='ok' for _,body in blocks)
 assert not re.search(r'^warning:',text,re.M)
 results.append({**r,'verified_test_names':[x[0] for x in blocks]})
platform=(E/'platform-movie.log').read_text();app=(E/'app-native-host.log').read_text()
frames=re.findall(r'composition full range positive=(true|false) PTS=([0-9.]+) size=64x48 RGBA_SHA256=([a-f0-9]{64})',platform)
assert len(frames)==6
control=[(float(t),h) for positive,t,h in frames if positive=='false']
positive=[(float(t),h) for positive,t,h in frames if positive=='true']
assert [x[0] for x in control]==[0.,0.1,0.2] and [x[0] for x in positive]==[0.1,0.2,0.3]
assert [x[1] for x in control]==[x[1] for x in positive]
for i,(_,digest) in enumerate(positive):
 assert digest==sha(repo/f'rust/docs/validation/native-positive-composition-2026-10-09/reader-output/zero-origin-decoded-{i}.rgba')
assert 'composition seek positive=true request=0.05 actual=0.1 exact_full_range_RGBA=true' in platform
assert 'native host composition trim_start=0.05 request=0.05 actual=0.1 exact_full_range_RGBA=true' in app
assert 'native host composition export trim=0.05..0.35 count=3 size=64x48 delays_cs=[10, 10, 10] one_shot=true' in app
assert 'decoded GIF pair equals exactly; cancellation/identity preserve prior bytes' in app
assert 'DelayedFirst, PTS=[0.0, 0.1, 0.2, 0.3]' in app and 'leading repeats first=false, opaque black=true' in app
assert json.loads((E/'format-final-result.json').read_text())['exit_code']==0
lint={}
for name,expected in [('strict-clippy',101),('base-strict-clippy',101),('scoped-clippy',101),('base-scoped-clippy',101),('platform-scoped-clippy',0)]:
 r=json.loads((E/(name+'-result.json')).read_text());log=E/(name+'.log')
 assert r['exit_code']==expected and sha(log)==r['log_sha256']
 diagnostics=re.findall(r'^(error: [^\n]+)\n\s*--> ([^\n]+)',log.read_text(),re.M)
 lint[name]={**r,'diagnostics':diagnostics}
assert Counter(map(tuple,lint['strict-clippy']['diagnostics']))==Counter(map(tuple,lint['base-strict-clippy']['diagnostics']))
assert Counter(map(tuple,lint['scoped-clippy']['diagnostics']))==Counter(map(tuple,lint['base-scoped-clippy']['diagnostics']))
assert len(lint['strict-clippy']['diagnostics'])==1 and len(lint['scoped-clippy']['diagnostics'])==7
for label in ['strict-clippy','scoped-clippy']:
 for _,location in lint[label]['diagnostics']:
  path='rust/'+location.rsplit(':',2)[0]
  assert path not in owned
  assert sha(repo/path)==next(x['sha256'] for x in base if x['path']==path)
postimages={path:sha(repo/path) for path in sorted(owned)}
(E/'source-postimages.json').write_text(json.dumps(postimages,indent=2)+'\n')
subprocess.run(['git','diff','--check'],cwd=repo,check=True)
summary={'base':setup['base'],'base_tree':setup['tree'],'verified_at':datetime.datetime.now(datetime.timezone.utc).isoformat(),'base_files':1533,'unchanged_base_files_and_modes':unchanged,'only_modified_base_paths':sorted(owned-{p for p in owned if '/fixture_media/' in p}),'build_files_unchanged_since_native_tests':1535,'build_manifest_sha256':sha(E/'build-source-before.json'),'fixture_provenance':json.loads((E/'fixture-provenance.json').read_text()),'source_postimages':postimages,'production_movie_section_unchanged':True,'production_native_gate_closed':True,'passed':76,'failed_tests':0,'ignored_tests':0,'test_results':results,'format_passed':True,'strict_clippy_passed':False,'lint':lint,'native_observations':{'control_pts':[x[0] for x in control],'positive_pts':[x[0] for x in positive],'same_host_rgba_sha256':[x[1] for x in positive],'before_first_request':0.05,'actual_first_following':0.1,'export_trim':[0.05,0.35],'gif_delays_centiseconds':[10,10,10],'existing_delayed_first_black_pts0_control_preserved':True},'limits':'Generated media/native CLI only; same-host decoded control equality, no pre-encoding/cross-host color equivalence, actual GUI, capture/TCC/AX, provider/key/vault, release or production permission acceptance. Strict Clippy failures reproduced on unchanged base files; no production fixes or lint suppression added.'}
(E/'final-validation.json').write_text(json.dumps(summary,indent=2)+'\n')
print(json.dumps({'passed':76,'unchanged_base_files':unchanged,'manifest_sha256':summary['build_manifest_sha256'],'final_validation_sha256':sha(E/'final-validation.json'),'strict_clippy_passed':False,'platform_scoped_clippy_passed':True,'source_postimages':postimages}))
