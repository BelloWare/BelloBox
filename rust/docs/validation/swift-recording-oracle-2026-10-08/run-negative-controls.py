import argparse, hashlib, json, subprocess
from pathlib import Path
parser=argparse.ArgumentParser();parser.add_argument('--repository', type=Path, required=True);parser.add_argument('--evidence',type=Path,required=True);args=parser.parse_args();stage=args.repository.resolve();evidence=args.evidence.resolve();evidence.mkdir(parents=True,exist_ok=True)
raw=stage/'rust/crates/bello-platform/src/recording/tests/raw_oracle.rs';swift=stage/'rust/crates/bello-platform/src/recording/tests/swift_reference.rs';original=stage/'BelloBox/Recording/GIF/GIFTranscoder.swift'
driver=stage/'rust/crates/bello-platform/src/recording/tests/swift_reference.swift'
originals={p:p.read_bytes() for p in [raw,swift,original,driver]}
controls=[
('permit_rgb_error_one',raw,'if actual != wanted {','if actual.abs_diff(wanted) > u8::from(offset % 4 != 3) {'),
('permit_alpha_error_one',raw,'if actual != wanted {','if offset % 4 != 3 && actual != wanted {'),
('bypass_pixel_comparison',raw,'if actual != wanted {','if false && actual != wanted {'),
('permit_pts_slack',raw,'if !pts.is_finite() || pts != expected.presentation_seconds {','if !pts.is_finite() || (pts - expected.presentation_seconds).abs() >= 0.001 {'),
('bypass_full_source_binding',swift,'if digest(original) != FULL_HASH {','if false && digest(original) != FULL_HASH {'),
('bypass_excerpt_binding',swift,'if digest(text) != hash {','if false && digest(text) != hash {'),
('changed_production_swift',original,'context.interpolationQuality = .medium','context.interpolationQuality = .high'),
('bypass_frame_count',swift,'if count != fixture::FRAME_COUNT {','if false && count != fixture::FRAME_COUNT {'),
('missing_driver_substitution',driver,'/* ORIGINAL_RENDERER */','/* MISSING_RENDERER */'),
('bypass_protocol_dimensions',swift,'if (width, height, length) != (fixture::WIDTH, fixture::HEIGHT, FRAME_BYTES)','if false && (width, height, length) != (fixture::WIDTH, fixture::HEIGHT, FRAME_BYTES)'),
('bypass_protocol_alpha',swift,'if rgba.chunks_exact(4).any(|pixel| pixel[3] != 255) {','if false && rgba.chunks_exact(4).any(|pixel| pixel[3] != 255) {'),
('bypass_raw_payload',raw,'if pixel[3] != 255 || payload[offset..offset + 3] != pixel[..3] {','if false && (pixel[3] != 255 || payload[offset..offset + 3] != pixel[..3]) {'),
('bypass_rgba_length',raw,'if rgba.len() != expected.rgba.len() {','if false && rgba.len() != expected.rgba.len() {'),
('runner_without_timeout',swift,'Instant::now() < deadline,','true || Instant::now() < deadline,'),
('runner_without_stderr_abort',swift,'fs::metadata(&log_path).unwrap().len() <= stderr_limit,','true || fs::metadata(&log_path).unwrap().len() <= stderr_limit,'),
('runner_kills_parent_only',swift,'libc::kill(-(self.0.id() as i32), libc::SIGKILL);','libc::kill(self.0.id() as i32, libc::SIGKILL);'),
]
results=[]
cmd=['cargo','test','--manifest-path',str(stage/'rust/Cargo.toml'),'--locked','--offline','-p','bello-platform','recording::tests::','--','--test-threads=1']
try:
 for name,p,old,new in controls:
  before=originals[p].decode();assert before.count(old)==1,name;p.write_text(before.replace(old,new))
  selected=cmd if name.startswith('runner_') else cmd+['--skip','recording::tests::swift_reference::swift_driver']
  result=subprocess.run(selected,stdout=subprocess.PIPE,stderr=subprocess.STDOUT);out=result.stdout.decode();(evidence/f'negative-{name}.log').write_text(out);detected=result.returncode!=0 and 'test result: FAILED' in out
  results.append({'control':name,'detected':detected,'exit_code':result.returncode,'mutated_path':str(p.relative_to(stage)),'mutated_sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'test_summary':[line for line in out.splitlines() if line.startswith('test result:')]});p.write_bytes(originals[p]);print(name,detected,flush=True);assert detected,name
finally:
 for p,data in originals.items():p.write_bytes(data)
 (evidence/'negative-controls.json').write_text(json.dumps({'controls':results,'restored_sha256':{str(p.relative_to(stage)):hashlib.sha256(p.read_bytes()).hexdigest() for p in originals}},indent=2)+'\n')
