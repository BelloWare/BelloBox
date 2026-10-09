from pathlib import Path
import datetime, hashlib, json, os, re, signal, subprocess, time

ROOT = Path(__file__).resolve().parent.parent
EVIDENCE = ROOT / 'evidence'
CONFIG = json.loads((EVIDENCE / 'commands.json').read_text())
ENV = os.environ.copy()
removed = [k for k in ENV if k.startswith(('BELLOBOX_', 'BELLO_AGENT_', 'BELLO_JOURNAL_', 'OPENAI_', 'ANTHROPIC_')) or k in ('AZURE_OPENAI_API_KEY', 'GEMINI_API_KEY', 'GOOGLE_API_KEY', 'LITELLM_API_KEY')]
for k in removed:
    ENV.pop(k)
isolated = ROOT / 'isolated-config'
isolated.mkdir(exist_ok=True)
ENV.update({'CARGO_TARGET_DIR': CONFIG['target'], 'CARGO_BUILD_JOBS': '2', 'MACOSX_DEPLOYMENT_TARGET': '14.0', 'RUST_BACKTRACE': '1', 'BELLOBOX_CONFIG_DIR': str(isolated)})
assert not ENV.get('CARGO_BUILD_TARGET')
assert not ENV.get('RUSTFLAGS') and not ENV.get('CARGO_ENCODED_RUSTFLAGS')
def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def stamp():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()

host = {'checked_at': stamp(), 'commands': [], 'removed_environment_variable_names': sorted(removed), 'build_environment': {k: ENV.get(k) for k in ['CARGO_TARGET_DIR', 'CARGO_BUILD_JOBS', 'CARGO_BUILD_TARGET', 'MACOSX_DEPLOYMENT_TARGET', 'RUST_BACKTRACE', 'RUSTC', 'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'DEVELOPER_DIR', 'BELLOBOX_CONFIG_DIR']}}
for command in [['/usr/bin/sw_vers'], ['/usr/bin/uname', '-m'], ['/opt/homebrew/bin/rustc', '-vV'], ['/opt/homebrew/bin/rustc', '--print', 'cfg'], ['/opt/homebrew/bin/cargo', '-V'], ['/usr/bin/xcrun', 'swiftc', '--version'], ['/usr/bin/xcrun', '--sdk', 'macosx', '--show-sdk-version']]:
    p = subprocess.run(command, capture_output=True, text=True, env=ENV, check=True)
    host['commands'].append({'command': command, 'stdout': p.stdout, 'stderr': p.stderr, 'exit_code': p.returncode})
assert 'target_os="macos"' in host['commands'][3]['stdout']
(EVIDENCE / 'host-toolchain.json').write_text(json.dumps(host, indent=2) + '\n')
clean = ['/opt/homebrew/bin/cargo', 'clean', '--offline', '--locked', '-p', 'bello-workbench-ui']
p = subprocess.run(clean, cwd=ROOT / 'source/rust', capture_output=True, env=ENV)
(EVIDENCE / 'clean-cache.log').write_bytes(p.stdout + p.stderr)
(EVIDENCE / 'clean-cache.json').write_text(json.dumps({'command': clean, 'exit_code': p.returncode, 'log_sha256': sha(EVIDENCE / 'clean-cache.log')}, indent=2) + '\n')
p.check_returncode()

for step in CONFIG['steps']:
    name = step['name']
    source = ROOT / step['source']
    resultpath = EVIDENCE / (name + '-result.json')
    if resultpath.exists():
        raise RuntimeError('Refusing to overwrite ' + name)
    if step['kind'] == 'test':
        inventory = json.loads((EVIDENCE / (step['inventory'] + '-result.json')).read_text())
        assert inventory['exit_code'] == 0
        selected = sorted(n for n in inventory['listed_tests'] if step['selector'] in n)
        assert len(selected) == step['expected_selected_count'], (name, selected)
        ignored = sorted(n for n in selected if n.split('::')[-1] in step['ignored_suffixes'])
        assert len(ignored) == len(step['ignored_suffixes']), (name, ignored)
        expected = sorted(set(selected) - set(ignored))
        if 'source_expected_tests' in step:
            assert expected == step['source_expected_tests']
        step = {**step, 'expected_tests': expected, 'expected_ignored_names': ignored, 'expected_selected_names': selected, 'expected_passed': len(expected), 'expected_ignored': len(ignored)}
    log = EVIDENCE / (name + '.log')
    current = {**step, 'started_at': stamp(), 'cwd': str(source / 'rust'), 'log': str(log)}
    (EVIDENCE / 'status.json').write_text(json.dumps({**current, 'state': 'running'}, indent=2) + '\n')
    print(json.dumps({'started': name, 'command': step['command']}), flush=True)
    start = time.monotonic()
    with log.open('xb') as stream:
        child = subprocess.Popen(step['command'], cwd=source / 'rust', env=ENV, stdout=stream, stderr=subprocess.STDOUT, start_new_session=True)
        try:
            rc = child.wait(timeout=1800)
        except subprocess.TimeoutExpired:
            os.killpg(child.pid, signal.SIGKILL)
            child.wait()
            rc = 124
    content = log.read_text(errors='replace')
    counts = [dict(zip(['passed', 'failed', 'ignored', 'measured', 'filtered_out'], map(int, m))) for m in re.findall(r'test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out', content)]
    binarypaths = re.findall(r'Running unittests .*?\(([^()]+)\)', content)
    if rc == 0 and step['kind'] == 'build':
        binarypaths.append(str(Path(CONFIG['target']) / 'debug/libbello_workbench_ui.rlib'))
    binaries = []
    for rel in binarypaths:
        path = Path(rel) if Path(rel).is_absolute() else source / 'rust' / rel
        if path.exists():
            binaries.append({'path': str(path), 'sha256': sha(path), 'file': subprocess.check_output(['/usr/bin/file', str(path)], text=True).strip()})
    tests = []
    for m in re.finditer(r'^test (\S+) \.\.\. ([^\n]*)', content, re.M):
        following = content[m.end():]
        immediate = m[2].strip()
        state = immediate if immediate == 'ok' or immediate == 'FAILED' or immediate.startswith('ignored') else None
        if state is None:
            ending = re.search(r'^(ok|FAILED|ignored[^\n]*)$', following, re.M)
            if ending:
                state = ending[1]
        if state is not None:
            tests.append((m[1], state))
    listed_tests = re.findall(r'^(\S+): test$', content, re.M)
    expected_match = None if step['kind'] != 'test' else sorted(n for n, state in tests if state == 'ok') == step['expected_tests']
    result = {**current, 'ended_at': stamp(), 'elapsed_seconds': time.monotonic() - start, 'exit_code': rc, 'log_sha256': sha(log), 'counts': counts, 'binaries': binaries, 'tests': tests, 'listed_tests': listed_tests, 'expected_name_set_matches': expected_match}
    resultpath.write_text(json.dumps(result, indent=2) + '\n')
    (EVIDENCE / 'status.json').write_text(json.dumps({**result, 'state': 'passed' if rc == 0 else 'failed'}, indent=2) + '\n')
    print(json.dumps({'finished': name, 'exit_code': rc, 'counts': counts, 'expected_name_set_matches': expected_match, 'elapsed_seconds': result['elapsed_seconds']}), flush=True)
    # Preserve failures and still run the independent requested configurations.
(EVIDENCE / 'status.json').write_text(json.dumps({'state': 'all-requested-commands-completed', 'checked_at': stamp()}, indent=2) + '\n')
