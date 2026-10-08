# Root integration validation

PR3 head 7b9a99ae7c19b8251836cc82b6618101e8f8a55e (tree de876ac8da9860ce584e815aff50be5cd46f4186) is preserved as the parent of this integration. Its ancestor is published abd7247c25c1736f7a4fd6a34032e7af9d7a1f5d.

Root independently reviewed the exact source/evidence and reproduced all445 core tests. Strict Rust1.99 Clippy found the new constant chunks_exact lint. The sole product change from the PR is gif_decode.rs:121 using as_chunks::<4>().0.iter() instead of chunks_exact(4). The rows are width*4 bytes, so behavior is identical; as_chunks is stable since Rust1.88 and compatible with peer1.91.1. No lint suppression or dependency change. Original peer source manifests and native receipts remain unmodified; the addendum binds the final postimage.

Final Linux results:445 core tests,50 App converter tests, strict core and App movie-fixtures all-target Clippy, formatting all passed. Existing proc-macro-error2 future-incompatibility notice remains. Native peer results on original PR source:49 focused core GIF and52 App converter tests, including both unchanged native-host cases. Native strict Clippy has a preserved baseline settings.rs needless_return failure; only the explicitly allowed-lint diagnostic passed there. No final-postimage native execution is claimed by root.

The original native mirrored export failure and diagnostics remain preserved. This fixes bounded strict GIF completion and exported-result preview. Production movie admission remains closed; no real-media, native GUI, capture/TCC/AX/Keychain/signing/performance/release acceptance is implied.

Rust product counts:56,866 production /39,744 test-support /105 benchmark. The two standalone Rust evidence probes total115 nonblank lines separately excluded from product counts. Shared code is counted once.
