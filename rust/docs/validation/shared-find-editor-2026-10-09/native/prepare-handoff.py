from pathlib import Path
import base64, hashlib, json, lzma

ROOT = Path(__file__).resolve().parent.parent
E = ROOT / 'evidence'
V = json.loads((E / 'validation.json').read_text())
assert V['all_requested_checks_passed']
assert V['tests']['passed'] == 37 and V['tests']['failed'] == V['tests']['ignored'] == 0

def sha(data):
    return hashlib.sha256(data).hexdigest()

files = {}
for p in sorted(E.iterdir()):
    if not p.is_file() or p.suffix not in ['.json', '.log', '.py']:
        continue
    if p.name == 'source-baseline.json' or p.name.startswith('handoff'):
        continue
    text = p.read_text()
    files[p.name] = {'sha256': sha(text.encode()), 'text': text}
payload = json.dumps({'format': 'bello-native-evidence-v1', 'candidate': V['identity']['commit'], 'files': files}, separators=(',', ':'), ensure_ascii=False).encode()
compressed = lzma.compress(payload, preset=9)
encoded = base64.b64encode(compressed).decode()
assert lzma.decompress(base64.b64decode(encoded)) == payload
for entry in json.loads(payload)['files'].values():
    assert sha(entry['text'].encode()) == entry['sha256']
(E / 'handoff-evidence.json.xz').write_bytes(compressed)
(E / 'handoff-evidence.base64').write_text(encoded + '\n')
by = {r['name']: r for r in V['results']}
postimages = '\n'.join(x['sha256'] + '  ' + x['path'] for x in V['postimages'])
commands = '\n'.join('| `' + ' '.join(r['command']).replace('/opt/homebrew/bin/cargo', 'cargo') + '` | ' + str(r['exit_code']) + ' |' for r in V['results'])
diagnostics = {r['name']: r['warning_error_summary_lines'] for r in V['results'] if r['warning_error_summary_lines']}
warning_text = ('No compiler warning/error or Cargo future-incompatibility notice appeared in these native logs. The separately reported Linux proc-macro-error2 notice is not a native finding; no exact-parent warning comparison was needed.' if not diagnostics else 'Native diagnostic summary: `' + json.dumps(diagnostics) + '`.')
body = f'''migration-peer — completed the read-only native library preflight from #issuecomment-6074820878, acknowledged in #issuecomment-6074833831.

Verified detached HEAD `2e72ec51bd010a96624ca7326f9a9afcb1057686`, tree `c3af9d1030f8d65df98cd9ef1b41d7759ca5ea5f`, parent `15197a71d81c620bf60da18f0d066505e4c0dc99` in a fresh isolated checkout. Exactly the five requested shared-workbench paths differ from the parent. All five supplied SHA-256 postimages matched before testing and after validation; all 1925 tracked file contents, Git blobs and modes remain unchanged. Checkout is clean. Cargo.lock, popup source and Agent pin are unchanged.

**All requested native checks passed: 37 library tests, zero failed/ignored; format; strict all-targets Clippy; package clean; final ordinary library build.** The compiled native inventory also listed exactly 37 tests, and every name executed once using the requested two test threads. Test execution used the same Mach-O arm64 binary as the native inventory, SHA-256 `{by['library-tests']['binaries'][0]['sha256']}`.

Commands ran from the exact checkout's rust/ directory with offline locked dependencies. An additional initial `cargo clean --offline --locked -p bello-workbench-ui` completed with exit 0 before testing to avoid cross-checkout artifacts. The extra inventory command verifies native test names/counts.

| Command | Exit |
|---|---:|
{commands}

The requested final package clean preceded the ordinary `--lib` build, which freshly compiled bello-workbench-ui from this exact checkout. The resulting library artifact is libbello_workbench_ui.rlib, SHA-256 `{by['ordinary-library']['binaries'][0]['sha256']}`. This is a library preflight, not a whole-app native binary seal.

{warning_text}

Host/toolchain: macOS 14.8 (23J21), arm64 / aarch64-apple-darwin; Rust/Cargo 1.91.1, Swift 6.0.2, macOS SDK 15.1, deployment 14.0. Two Cargo jobs and isolated generated configuration; provider/fixture environment overrides were removed. Exact host outputs, commands, exits, durations, test names, binary/library hashes and raw text logs are in the lossless receipt below. Pinned CI remains a separate gate.

The passing synthetic tests cover exact original UTF-8/CRLF ranges and validation limits, retained edit history/selection/focus and Vim search, IME/drag Busy, explicit select_all, cached-child set/clear invalidation, same-length revision-reset replacement, clipped/wrapped scene-paint geometry, prepaint exclusion, cancellation and stale deferred receipts. These are GPUI TestPlatform results. Actual AppKit/AX/IME/visibility acceptance and the Agent consumer's full Find workflow remain open; the consumer must fence outer navigation/lifetime/geometry and bound no-receipt retries.

No source changes, assertion relaxation, permission changes, screenshots, binary uploads, real credentials or user-device access occurred. Shared rust and main were not moved. The original checkout remains clean at59339af1fa422acea4067207c55344666c3fa4ab. Root remains the integrator; no repair was needed for this candidate.

Verified postimages:
```text
{postimages}
```
Source manifest SHA-256 `{V['source_manifest_sha256']}` (1925 tracked files; full manifest retained locally). Validation JSON SHA-256 `{sha((E / 'validation.json').read_bytes())}`.

Lossless xz/base64 JSON: files[name]={{sha256,text}}. Decode base64, decompress xz and verify each UTF-8 file. Contains {len(files)} text records, including unabridged logs, checkout/source verification, commands and runner/finalizer scripts. No binaries/screenshots embedded. Compressed SHA-256 `{sha(compressed)}`; decoded JSON SHA-256 `{sha(payload)}`.

<details><summary>Native library evidence</summary>

```text
{encoded}
```
</details>
'''
body = body.replace('at59339', 'at 59339')
assert len(body) < 64000, len(body)
(E / 'handoff.md').write_text(body)
metadata = {'chars': len(body), 'body_sha256': sha(body.encode()), 'compressed_sha256': sha(compressed), 'decoded_sha256': sha(payload), 'files': len(files)}
(E / 'handoff-metadata.json').write_text(json.dumps(metadata, indent=2) + '\n')
print(json.dumps(metadata))
