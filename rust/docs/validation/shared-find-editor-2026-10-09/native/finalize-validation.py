from pathlib import Path
import datetime, hashlib, json, os, re, subprocess

ROOT = Path(__file__).resolve().parent.parent
E = ROOT / 'evidence'
SOURCE = ROOT / 'source'
PREP = json.loads((E / 'preparation.json').read_text())

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def git(*args, cwd=SOURCE):
    return subprocess.check_output(['git', *args], cwd=cwd, text=True).strip()

def put(path, value):
    path.write_text(json.dumps(value, indent=2) + '\n')

assert git('rev-parse', 'HEAD') == PREP['commit']
assert git('rev-parse', 'HEAD^{tree}') == PREP['tree']
assert git('rev-parse', 'HEAD^') == PREP['parent']
assert not git('status', '--porcelain')
entries = json.loads((E / 'source-baseline.json').read_text())
for x in entries:
    p = SOURCE / x['path']
    data = os.readlink(p).encode() if p.is_symlink() else p.read_bytes()
    actual = {'path': x['path'], 'mode': '120000' if p.is_symlink() else ('100755' if p.stat().st_mode & 0o111 else '100644'), 'git_blob': hashlib.sha1(b'blob ' + str(len(data)).encode() + b'\0' + data).hexdigest(), 'sha256': hashlib.sha256(data).hexdigest()}
    assert actual == x, x['path']
assert git('rev-parse', 'HEAD', cwd=PREP['original_repo']) == PREP['original_repo_head']
assert git('status', '--short', cwd=PREP['original_repo']) == PREP['original_repo_status']
results = []
for step in json.loads((E / 'commands.json').read_text())['steps']:
    r = json.loads((E / (step['name'] + '-result.json')).read_text())
    assert r['command'] == step['command'] and r['log_sha256'] == sha(r['log'])
    content = Path(r['log']).read_text(errors='replace')
    r['compiler_diagnostics'] = []
    for m in re.finditer(r'^(warning|error)(?:\[([^\]]+)\])?: ([^\n]+)\n\s*--> (.*?):(\d+):(\d+)\s*$', content, re.M):
        p = SOURCE / 'rust' / m[4]
        r['compiler_diagnostics'].append({'level': m[1], 'code': m[2], 'message': m[3], 'file': m[4], 'line': int(m[5]), 'column': int(m[6]), 'source_anchor': p.read_text().splitlines()[int(m[5]) - 1].strip() if p.exists() else None})
    r['warning_error_summary_lines'] = [line for line in content.splitlines() if re.match(r'^(warning|error)(\[|:)', line)]
    r['future_incompatibility_notice_lines'] = [line for line in content.splitlines() if 'future version of Rust' in line or 'future-incompat' in line]
    r['compile_and_run_lines'] = [line for line in content.splitlines() if 'Compiling bello-workbench-ui ' in line or 'Checking bello-workbench-ui ' in line or 'Running unittests ' in line]
    results.append(r)

by = {r['name']: r for r in results}
initial_clean = json.loads((E / 'clean-cache.json').read_text())
assert initial_clean['exit_code'] == 0 and initial_clean['log_sha256'] == sha(E / 'clean-cache.log')
inventory, tests, build = by['inventory'], by['library-tests'], by['ordinary-library']
assert inventory['exit_code'] == 0
assert len(inventory['listed_tests']) == 37
assert any('Compiling bello-workbench-ui ' in line and str(SOURCE / 'rust') in line for line in inventory['compile_and_run_lines'])
assert sorted(n for n, state in tests['tests']) == sorted(inventory['listed_tests'])
assert [b['sha256'] for b in tests['binaries']] == [b['sha256'] for b in inventory['binaries']]
assert all('Mach-O 64-bit executable arm64' in b['file'] for b in tests['binaries'])
assert by['clean-before-ordinary-library']['exit_code'] == 0
assert build['started_at'] >= by['clean-before-ordinary-library']['ended_at']
if build['exit_code'] == 0:
    assert any('Compiling bello-workbench-ui ' in line and str(SOURCE / 'rust') in line for line in build['compile_and_run_lines'])
    assert len(build['binaries']) == 1
    assert build['binaries'][0]['path'].endswith('/libbello_workbench_ui.rlib')
    assert build['binaries'][0]['sha256'] == sha(build['binaries'][0]['path'])

postimages = json.loads((E / 'candidate-postimages.json').read_text())
assert {x['path']: x['sha256'] for x in postimages} == PREP['postimages']
v = {
    'identity': PREP,
    'verified_at': datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'source_files_and_modes_verified_before_and_after': len(entries),
    'source_manifest_sha256': sha(E / 'source-baseline.json'),
    'postimages': postimages,
    'postimages_sha256': sha(E / 'candidate-postimages.json'),
    'source_checkout_clean': True,
    'original_checkout_unchanged': True,
    'source_edits': False,
    'commands_completed': len(results),
    'all_requested_checks_passed': all(r['exit_code'] == 0 for r in results),
    'format_passed': by['format']['exit_code'] == 0,
    'tests': tests['counts'][-1],
    'native_test_count_matches_linux_report': len(inventory['listed_tests']) == 37,
    'exact_native_inventory_matches_executed_tests': True,
    'test_binary_matches_native_inventory': True,
    'strict_clippy_passed': by['strict-clippy']['exit_code'] == 0,
    'ordinary_library_build_passed': build['exit_code'] == 0,
    'library_package_cleaned_before_tests_and_final_ordinary_build': True,
    'actual_candidate_compiled_for_tests_and_final_build': True,
    'host': json.loads((E / 'host-toolchain.json').read_text()),
    'results': results,
    'scope': 'Read-only exact candidate native library preflight on the approved CLI/TestPlatform host. Synthetic GPUI scene paint and generated editor text only. No native GUI binary seal, AppKit/AX/real IME/visibility acceptance, screenshots, binary uploads, real credentials/user-device access, permission changes, source edits, shared rust/main publication or installation. Root integrates separately. Agent Find consumer navigation/lifetime/geometry fences, bounded no-receipt retries and full workflow validation remain separate; pinned CI remains a separate gate.'
}
put(E / 'validation.json', v)
print(json.dumps({'head': PREP['commit'], 'tree': PREP['tree'], 'all_checks_passed': v['all_requested_checks_passed'], 'tests': v['tests'], 'format': v['format_passed'], 'strict_clippy': v['strict_clippy_passed'], 'ordinary_library': v['ordinary_library_build_passed'], 'library_artifacts': build['binaries'], 'diagnostics': {r['name']: r['warning_error_summary_lines'] for r in results}, 'validation_sha256': sha(E / 'validation.json')}))
