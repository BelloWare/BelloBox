# Interactive cloud provider Settings validation

Date: 2026-10-08, approximately 20:30–20:35 UTC.
Published source: e9cdb40c6475a9ba006cd245050fc650a82780ba.
Binary SHA-256: 01e32b7141c9816b73cef9a95a01955db6d03cc459e7f18e21aaf1e178d8ed4f.

## Scope and provenance

This was actual native GPUI UI interaction through cloud CUA on Linux/X11 and Mesa lavapipe. It was not a GPUI test host, Mac runtime acceptance, paid provider test, native permission test, or credential/vault test. The user's computer was not used. No application source edits or remote writes were performed.

An ordinary App binary was built using cargo build --manifest-path rust/Cargo.toml -p bellobox-app --locked from /workspace/shared/box-provider-quit-fix. The checkout HEAD was the older dec2411 base with applied source changes; its 192 tracked crate/Cargo source files were byte-compared against published e9cdb40 and all matched. source-manifest.json records their SHA-256 hashes. Compiler lane was coordinated with Clock owner and released immediately after the 8.39-second build. build.log records the build; the only warning is the existing proc-macro-error2 future-incompatibility warning.

The old qa.sh and old binary were not used. launch.sh started this independently copied binary, numeric loopback fixture server, isolated config directory and synthetic-gui-key runtime key. No provider/endpoint/model process overrides were set. BELLOBOX_SETTINGS_CATEGORY=ai selects the initial Settings category but does not open or drive controls. Source runtime setup UI and transport were otherwise unchanged.

## Verified results

1. Actual Home Settings input opened AI Provider. Screenshot 01 shows the isolated endpoint http://127.0.0.1:38417/v1 and before-gui model. No request log existed before explicit Load.
2. Explicit Load produced GET /v1/models and visible 'Loaded 2 models. Choose a model.' (04 transient Loading, 05 result).
3. Actual model dropdown showed gui-other-model and gui-selected-model (06). Clicking gui-selected-model changed the field and model behavior panel (07) and persisted that model to settings.json. config-before.json was captured before this selection; config-after-select.json and config-final.json preserve the result. No synthetic key is present in saved config.
4. Explicit Test connection succeeded (08) with 'Connected: Hello from GUI fixture'. Request log records POST /v1/chat/completions, model gui-selected-model, stream true, max_completion_tokens 4096, exact user text 'Reply with a short, friendly hello.' and no selected-text wrapper.
5. A second explicit Test was held server-side before response headers. admitted-held proves the request reached the server. Screenshot 09 shows Testing model and Cancel request. There were exactly three requests total: one Load and two Tests.
6. While the second Test remained held, multiple Ctrl+Q attempts did not terminate either App window. No visible Quit-refusal dialog appeared. This is an observed nontermination result, not proof that every attempted Quit was delivered or that refusal presentation works.
7. Releasing the fixture hold returned the UI to Connected without automatic exit. Ctrl+A in the focused model field visibly selected its text (15), establishing keyboard delivery for that control. A fresh Ctrl+Q then closed both BelloBox windows; 16-retired-quit-desktop.png shows the returned terminal. The launch shell exit status was 0, captured as exit-code before any subsequent terminal command.

## Exact input observations and limitations

- Terminal typeText and Sky type_text failed with 'window does not expose a Paste action'. The launch command was entered through documented Sky press_key calls, one character at a time, then Return.
- Home Settings: initial Sky global click (159,760) did not immediately open it; a bound-window click at local (72,643), held 200 ms, did open Settings.
- Settings Load: bound local click (1112,467), held 200 ms, did not dispatch. Moving global pointer to (1205,584) and another bound click also did not dispatch. A subsequent direct Sky global click at (1205,584) dispatched Load. Screenshots 02/03 document unsuccessful attempts; 04/05 show real dispatch/result. These differences are not attributed to a proven application bug.
- Subsequent successful menu interactions used explicit Sky pointer move then direct global click: dropdown (1156,584), selected entry (1020,649), Test (391,900). Scrolling down 352 pixels revealed complete Test result.
- Second Test used global (390,777). Quit attempts used Sky Control_L+q, bound settings.pressKey('ctrl+q'), and focused-title/field retries. One duration 0.2 call was rejected by the tool as non-integer; a 200 ms retry executed. No refusal appeared in screenshots 10–14; their filenames are chronological action labels and do not certify a blocked-Quit dialog.
- After field click at global (640,457), Ctrl+A selected model text. The fresh post-retirement Ctrl+Q did exit, so keyboard transport can work; the precise reason earlier attempts showed no dialog remains undiagnosed.
- Full window dimensions were 1178x814 at desktop client origin (93,117). Bound screenshots use local coordinates; full desktop screenshots include window chrome. All PNGs are unmodified originals.
- Source selection invalidation and Quit ownership automated tests remain separate evidence. This run does not upgrade their claims to visual refusal acceptance. No last-window-close, native Dock/OS quit, Anthropic/Responses, Ask AI GUI or manual text editing acceptance is claimed.

## Artifact inventory

Original PNGs: 01 through 16 as present. requests.jsonl contains only the three synthetic requests; config snapshots contain only isolated test preferences. server.py/launch.sh reproduce the harness. build.log, app.log, server.log, source-diff.txt (empty), source-manifest.json and exit-code preserve provenance and outcomes. bellobox-e9 is the exact executed ordinary binary. Checksums are in evidence-sha256.json. Root owns any later publication.
