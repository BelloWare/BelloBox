# Actual cloud Linux QR acceptance, 2026-10-09

Binary and source are bound in manifest-c301cc213784.json. All UI actions used CUA Sky on cloud X11 desktop, software Vulkan runtime sourced from existing gui-runtime-env.sh. Ordinary no-fixture build. No Mac, permissions, security settings, global portal service, source edits, remote writes or screenshot publication.

Observed pass:
- Home Ctrl+K, qr search, Tab enters empty editor. Empty QR blank, Save/Copy disabled.
- Actual typing edited-qr generates QR; Enlarge held Return1200ms toggles once to Compact; Return returns compact.
- Forward focus (visible rings/caret): editor -> Enlarge -> Paste -> Clear -> Save -> Copy Image -> Open -> search -> editor. Shift-Tab reverses entire cycle.
- Select/copy editor text, Clear, explicit Paste restores edited-qr. Clear yields0 bytes. Open edited-qr gives full popup with exactly edited-qr.
- BELLOBOX_TOOL=qr/BELLOBOX_INPUT=synthetic-original route presents18-byte original; full popup requires actual pointer editor focus before Ctrl+K (baseline). Palette inherits original.
- Edit QR to edited-qr; search Text Tools shows18-character original preview. Search qr again restores9-byte edited draft. Separate actual Open Text Tools after edited-again gives synthetic-original input and SYNTHETIC-ORIGINAL output.
- External Mousepad fixture1001é copied through native Ctrl+A/C (editor pointer focus required), explicit palette Paste accepts draft and rejects QR encoding:2002/2000 UTF-8 bytes, Not encodable, blank image, Save/Copy disabled. Clear recovers empty; Open yields explicitly empty full QR popup, not original selection.
- Official isolated GTK FileChooser version4 available after restoration. Save opens actual GTK chooser. Escape cancels, palette remains, draft retained, existing PNG unchanged. Retry to snapshot.png succeeds.
- Held Return1200ms on Save opens chooser and its native autorepeat accepts default name; exactly one BelloBox QR.png created, no duplicate dialog/file. This is documented GTK chooser behavior, not a Mac finding.
- Saved click-snapshot PNG and retry snapshot.png are byte-identical522x522 grayscale PNG2827 bytes, sha256168af36df347fd8652022bd3a1f3032ba626bf109810271552b486ab813ba5a4. Only black/white pixels. Existing file preserved across cancellations/retry.

Failure/limit:
- Copy Image status says QR image copied, but external image/png target unavailable. GTK inspector only found TARGETS,SAVE_TARGETS,UTF8_STRING,text/plain variants. GPUI0.2.2 linux/x11/client.rs1555 sends set_text(item.text().unwrap_or_default()) and caches image internally; no external PNG success claim. Owner is preparing narrow truthful capability status fix; this initial receipt remains bound to original binary.
- GTK modality blocks actual underlying editor mutation while chooser pending. Tried moving chooser aside, clicking editor and typing; draft unchanged; Escape canceled chooser with palette retained. Save click-time content remains stable, but adversarial edit-while-dialog race was NOT independently exercised. Synthetic immutable-snapshot tests are separate.
- Existing output was hash-preserved; overwrite-confirmation path not accepted here.
- No scanner decoder installed (cv2/pyzbar/zxingcpp/zbar/quirc absent); no independent decode claim.
- No macOS attached-sheet/clipboard/IME/Spaces/security/release acceptance.

Portal prerequisite restoration:
- Actual desktop initial probe ServiceUnknown, portal executables absent.
- Existing signed Debian Release verified with system archive keyring via sqv (three valid fingerprints saved); Packages.xz SHA chain checked.
- Official packages xdg-desktop-portal1.20.3+ds-1 and xdg-desktop-portal-gtk1.15.3-1 from deb.debian.org matched exact documented SHA256 values in portal-plan.json, dpkg-deb extraction only to isolated workspace. No maintainer scripts or system install.
- Process-local dbus-run-session starts permission-store, GTK backend, portal frontend; default=none/FileChooser=gtk isolated config. Child cleanup trap used. Final global desktop absence probe pending session cleanup.

Post-run cleanup verified: Ctrl+Q closed all BelloBox windows, process-local trap returned Terminal. Original desktop bus again reports ServiceUnknown (desktop-after-c301.txt); no global portal registration persisted.

Root classified held Return Save autoaccept as an unintended modal handoff risk requiring correction. Initial behavior is NOT acceptance: owner will defer keyboard Save until activation key release. Retest new seal with held Return and require chooser stays open with no output until a distinct confirmation.
