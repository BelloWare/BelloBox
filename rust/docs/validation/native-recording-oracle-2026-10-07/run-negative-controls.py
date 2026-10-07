import hashlib, json, os, subprocess
from pathlib import Path
import argparse
parser=argparse.ArgumentParser()
parser.add_argument('--repository', type=Path, required=True)
parser.add_argument('--evidence', type=Path, required=True)
args=parser.parse_args()
stage=args.repository.resolve()
evidence=args.evidence.resolve()
evidence.mkdir(parents=True, exist_ok=True)
path=stage/'rust/crates/bello-platform/src/recording/tests/raw_oracle.rs'
serial=stage/'rust/crates/bello-platform/src/recording/tests.rs'
originals={p:p.read_bytes() for p in [path,serial]}
controls=[
 ('permit_rgb_error_two',path,'let allowance = u8::from(offset % 4 != 3);','let allowance = if offset % 4 != 3 { 2 } else { 0 };'),
 ('permit_alpha_error_one',path,'let allowance = u8::from(offset % 4 != 3);','let allowance = 1;'),
 ('bypass_pixel_comparison',path,'if actual.abs_diff(wanted) > allowance {','if false && actual.abs_diff(wanted) > allowance {'),
 ('bypass_pts_check',path,'if !pts.is_finite() || (pts - expected.presentation_seconds).abs() >= 0.001 {','if false && (!pts.is_finite() || (pts - expected.presentation_seconds).abs() >= 0.001) {'),
 ('bypass_dimensions_check',path,'if (width, height) != (expected.width, expected.height) {','if false && (width, height) != (expected.width, expected.height) {'),
 ('bypass_length_check',path,'if rgba.len() != expected.rgba.len() {','if false && rgba.len() != expected.rgba.len() {'),
 ('bypass_raw_payload_check',path,'if pixel[3] != 255 || payload[offset..offset + 3] != pixel[..3] {','if false && (pixel[3] != 255 || payload[offset..offset + 3] != pixel[..3]) {'),
 ('poison_cascades',serial,'mutex.lock().unwrap_or_else(|error| error.into_inner())','mutex.lock().unwrap()'),
]
results=[]
cmd=['cargo','test','--manifest-path',str(stage/'rust/Cargo.toml'),'--locked','--offline','-p','bello-platform','recording::tests::raw_oracle::','--','--test-threads=1']
try:
 for name,p,old,new in controls:
  before=originals[p].decode(); assert before.count(old)==1, name
  p.write_text(before.replace(old,new))
  result=subprocess.run(cmd,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
  out=result.stdout.decode(); (evidence/f'negative-{name}.log').write_text(out)
  detected=result.returncode!=0 and 'test result: FAILED' in out
  results.append({'control':name,'detected':detected,'exit_code':result.returncode,'mutated_sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'test_summary':[line for line in out.splitlines() if line.startswith('test result:')]})
  p.write_bytes(originals[p])
  print(name, detected, flush=True)
  assert detected, name
finally:
 for p,data in originals.items(): p.write_bytes(data)
 (evidence/'negative-controls.json').write_text(json.dumps({'controls':results,'restored_sha256':{str(p.relative_to(stage)):hashlib.sha256(p.read_bytes()).hexdigest() for p in originals}},indent=2)+'\n')
