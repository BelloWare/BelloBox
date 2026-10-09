# First-following movie fixture integration

Source candidate: 59339af1fa422acea4067207c55344666c3fa4ab, tree c6b021437434ce691c1a225469abeac3b92c80df; parent 5934b9897854a4102ef71d6463d4b4f7a3005158. PR: https://github.com/BelloWare/BelloBox/pull/6. This receipt adds documentation only to the exact reviewed source candidate.

The new sealed synthetic MOV fixtures exercise the existing Rust movie reader and GIF workflow when the first image has positive PTS. Request 0.05 returns actual PTS 0.1; full traces are [0.1,0.2,0.3] versus [0,0.1,0.2]. Existing half-frame GIF selection yields three equal decoded frames with delays [10,10,10]. Production admission, seek, decoding, timing, cancellation and publication implementations are unchanged. Ordinary MovieAsset::open remains unavailable. This is fixture/test-support progress, not production movie capture or user-media acceptance.

Independent source/evidence review verified all 41 checksums, six source postimages, 1535 build-input hashes and 1533 baseline files. Native peer evidence records 76 passes (24 platform,49 GIF,3 App), zero failures/ignored. Full native strict Clippy still fails on unchanged baseline settings needless_return and seven nonminimal_bool diagnostics; platform-only strict Clippy and formatting pass. No full native strict success is claimed.

Root Linux validation on the exact source candidate: platform movie 14 passed; core GIF49 passed; App converter50 passed; strict App movie-fixtures all-target Clippy passed. Native macOS tests are cfg-excluded on Linux. Existing proc-macro-error2 future-incompatibility notice retained. Before these checks the bellobox-app package was cleaned and zero surviving App executables verified, avoiding earlier unrelated negative-control test artifacts. No product mutations were made by this validation.

Counts: 56,866 production /40,009 test-support /105 benchmark nonblank physical Rust lines; support delta +265, production delta zero. Shared workbench counted once in BelloBox. Existing115 standalone evidence Rust lines remain separately excluded. Counts imply neither parity nor completion date.

No screenshots are published. Native production capture/TCC/AX/Keychain/signing/performance/release acceptance remain gated.
