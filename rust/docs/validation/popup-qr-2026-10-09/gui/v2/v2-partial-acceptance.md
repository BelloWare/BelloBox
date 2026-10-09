# Ordinary popup v2 actual cloud GUI: blocked

Observed 2026-10-09 UTC parent clock. Cloud desktop filesystem clock is offset (export mtime October 8 21:44); ordering here is based on actions, not mtime.

## Identity and environment

- Read immutable f1c21ae4 migration handoff, current palette-qr.md, full-popup HANDOFF.md and source-manifest-v2.json.
- All seven source manifest hashes matched on resumption.
- Ordinary frozen binary SHA256: d1b68a3f208c603977ee532f98edeaf4bd08dcaf5050fb1b84bdff76b39edc0a.
- Cloud Linux X11, Mesa software rendering using existing gui-runtime-env.sh; isolated dbus-run-session and existing officially extracted portal prerequisites. No global services, user Mac, source/Cargo/remote writes, credentials or spending.
- No earlier actual-GUI findings recovered: only manifest-check.json and launch recipe existed.
- CUA type_text on the terminal failed with missing Paste action; entered the launch command using documented press_key API. Duplicate old terminal identity was resolved by closing one unused terminal window. No launch failure attributed to product.

## Passed observations before blocker

- Popup started with exact 20-byte draft popup-qr-v2-sentinel.
- Copy Image was visibly disabled and status said Copy Image is unavailable on X11. Use Save… to export PNG. Explanation persisted alongside Save status.
- Explicit normal mouse Save opened actual GTK SaveFile portal; ordinary Save completed.
- exports/BelloBox QR.png is 3353 bytes, grayscale L, 528×528 pixels, SHA256 3ae3d4a72c51418e5abb57d19da7b9db514ee398ac051850be6eb47ef8209c74. This is filesystem/PNG evidence, not scanner validation.
- Footer/status were visible at both measured sizes.

## Blocking actual rendering failure

Exact client dimensions are independently read with xwininfo via the cloud Terminal, not inferred from decorated screenshot size. Window 0x1c00001, client origin (93,117).

- At client 400×480, decoration 410×514 at (88,88), preview approximately desktop (105,189)..(481,351). Black QR extends into Encoded text row y357..375 and is hidden/overpainted by editor starting y376. Lower finder is not visible. Reobserved after focus-away/back. Raw receipt windows-minimum.txt.
- At client 520×620, preview approximately desktop x105..601,y199..462. White inner image footprint protrudes to y481, through the Encoded text row y477..488. Black modules x251..454,y245..448 remain inside at this size. Raw receipt windows-preferred.txt.
- Local-only captures are outside the text evidence directory: ../captures/minimum-failure.jpg and ../captures/preferred-failure.jpg. Do not publish these captures.
- Sent exact evidence to root and integration source owner; candidate held. No source edits by this validator.

## Not established

Stopped acceptance on product blocker. No claim yet for explicit disabled-Copy clipboard sentinel preservation, editing/empty/over-limit transitions, Cancel/no-overwrite, held-key mouse refusal/recovery/no replay, long status scrolling, taller layout, or palette smoke. No actual decoder available/used. No macOS NSPasteboard/AppKit/IME/AX acceptance. Portable snapshot/clipboard tests remain distinct from this actual GUI evidence.
