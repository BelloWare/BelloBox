# Publication scope

The original README describes the complete local validation directory. This repository contains its 16 unmodified PNGs, synthetic request/config records, scripts, build/runtime logs, source manifest and exit status. The 138 MiB executable, runtime config directory and ephemeral endpoint file are deliberately not uploaded. Rebuild the exact recorded source to reproduce; the launch script's local binary and toolchain paths must be adapted to the validation host.

`evidence-sha256.json` covers the 31 original files published here. Screenshot `10-quit-blocked.png` is a chronological filename only: no visible refusal dialog was observed, as the README explicitly records. The verified interactive slice is Load, model selection/persistence and Test connection using numeric loopback. Held-work nontermination and no automatic exit are observations; native macOS/TCC/AX/Keychain/signing/performance and visible Quit-refusal acceptance remain separate.
