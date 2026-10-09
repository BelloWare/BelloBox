# Swift comparison alignment attribution

The `Region` / `Frontiers` alignment portion in
`crates/bellobox-core/src/developer/comparison.rs` is a modified Rust adaptation
of the Swift standard library's linear-space Myers implementation.

Original attribution retained in the source:

Copyright (c) 2015 - 2019 Apple Inc. and the Swift project authors

Licensed under Apache License v2.0 with Runtime Library Exception.

See https://swift.org/LICENSE.txt for license information and
https://swift.org/CONTRIBUTORS.txt for the list of Swift project authors.

The full, byte-exact upstream license, including the Runtime Library Exception,
is included in [licenses/swift-runtime-LICENSE.txt](licenses/swift-runtime-LICENSE.txt).
This notice concerns the adapted portions; it does not replace the licensing of
unrelated project code.

## Immutable provenance

Official repository: `swiftlang/swift`.

Inspected release tag: `swift-6.3.3-RELEASE`.

Tag target, verified through GitHub's official API on 2026-10-09:
`064859e41d68596f486c5d724401cb370f260409`.

- [Diffing.swift](https://github.com/swiftlang/swift/blob/064859e41d68596f486c5d724401cb370f260409/stdlib/public/core/Diffing.swift)
  - Git blob: `cd2a53a69cb75fd5506e388e56d974c0db2bce6b`
  - SHA-256: `0bee4637d036f97d22c29bd6a4f0c73b598bedc95f17978cddce5158025da285`
  - UTF-8 length: 21,681 bytes
- [LICENSE.txt](https://github.com/swiftlang/swift/blob/064859e41d68596f486c5d724401cb370f260409/LICENSE.txt)
  - Git blob: `61b0c78195f2d00acaf658000eeca6ad406a3a29`
  - SHA-256: `770af8291f708538d8ff885a0bbc4e045cd700531741c4f99528d435c14d7f55`
  - Included copy: 11,751 bytes; checked against the upstream Git blob
- [CharacterProperties.swift](https://github.com/swiftlang/swift/blob/064859e41d68596f486c5d724401cb370f260409/stdlib/public/core/CharacterProperties.swift)
  - Behavioral reference for first-scalar grapheme whitespace classification
  - Git blob: `ebbeba1e00436c8cf4d1e3cfefb26af97753dc38`
  - SHA-256: `ce6b3f45ee4e28e430736d8010211ab1dea5efd489af7173fbba874cff3298e8`
  - UTF-8 length: 12,351 bytes

The release-tag and immutable-commit file contents were compared byte for byte.
The upstream root tree at this commit contains `LICENSE.txt` and no root `NOTICE`
file. The original Diffing source header identifies its copyright and license.

## Modifications and validation scope

Modified on 2026-10-09 for this Rust migration: safe Rust vectors and slices,
NFC-interned integer token matching, two explicit edit masks, input/token/output
bounds, cancellation checks, direct handling of empty axes, and error results
instead of an unreachable trap. The source's descending-diagonal search,
bidirectional overlap choices, region processing order, and common-prefix/suffix
shrinking determine ambiguous repeated-token alignment.

Swift 6.2 used a different trace implementation. A minimal diff alone does not
prove the same displayed rows. The exact pinned-application native Swift oracle
must verify row order and copied UTF-8 against the executing macOS runtime.
Neither this source reference nor Linux tests claim equality across all Swift
runtimes or Unicode versions.

## Distribution

Keep the full upstream license and these attribution/modification notices with
redistributions of the adapted source and applicable binary distributions.
The Runtime Library Exception describes compiler-embedded software; this project
retains the notices for its source adaptation rather than assuming that exception
removes them. The preview packaging script now copies these notices, preserving
their relative layout under `Resources/ThirdPartyNotices`; its validator rejects
missing, modified, or symlinked notices. The 11 packaging-policy Python tests
pass locally. Exact-commit native macOS packaging acceptance remains pending CI.
This does not claim an existing release package was audited or alter signing,
notarization, or production-release gates.
