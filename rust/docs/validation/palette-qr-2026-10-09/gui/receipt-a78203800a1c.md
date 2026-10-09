# Final changed-behavior cloud GUI recheck

2026-10-09. Ordinary sealed binary /workspace/shared/box-palette-qr-sealed/bellobox-a78203800a1c, SHA256 d4702add5fe9b03b38ce71f98d6993d1adf9514559f8c6dd57ee88588be82069, source a78203800a1c5d462814991da4cf65e60141a672 tree5415e0154fdf369f28e83a89a8f56e5436135449. Hash verified immediately before launch. Same CUA cloud X11/software-Vulkan/private portal route; launch-a78203800a1c.sh and a78203800a1c/probe.txt bind launch/probe. No source/dependency modifications in retest.

Changed behavior PASS:
- Palette on synthetic-original valid QR visibly disables Copy Image and persistently says “Copy Image is unavailable on X11. Use Save… to export PNG.” No image-copied claim.
- Copied synthetic-original text as sentinel in editor, changed draft to new-draft, clicked disabled Copy, then Clear/Paste restored synthetic-original. Disabled action did not replace clipboard sentinel.
- Forward Tab order visibly editor -> Enlarge -> Paste -> Clear -> Save -> Open -> search, skipping unsupported Copy. Shift-Tab from search goes Open then Save.
- With release-save draft and Save focused, held Return1800ms leaves exactly one actual GTK Save chooser open after release, filename untouched, no autoaccept/new output. Prior two PNG hashes unchanged. Unlike initial c301 behavior, no accidental save occurs.
- Separate name entry release-save.png and a distinct Return confirms save. UI reports Saved path. Independent Pillow validates522x522 grayscale PNG2847 bytes, only black/white values, SHA2564d7f2e90a3defa3205ed6501b272540f1bc0a0c686f4665f3c01669e37f11818.
- Mouse Save opens chooser normally. Escape cancels and retains release-save draft and saved output.
- Held Space1500ms from focused Save likewise leaves chooser awaiting confirmation; Escape cancels without duplicate export.
- Persistent unsupported-copy footer remains after Save and Cancel status updates.
- Ctrl+Q closes all BelloBox windows and returns original Terminal; original desktop bus FileChooser probe again ServiceUnknown (desktop-after-a782.txt), no persistent global portal registered.

Limits and scope:
- CUA held-key operation is atomic to caller, so no intermediate screenshot while key remains physically down. Post-release chooser and no-autoaccept/output behavior are actually observed. No genuine simultaneous Return+Space overlap or mouse-during-held-key was attempted through undocumented input APIs. Those remain dispatched synthetic regression evidence, not actual desktop coverage.
- This is the changed palette Copy behavior. Baseline full QR popup still visibly has Copy Image; its platform capability presentation was not changed/retested in this narrow patch.
- Initial broader actual interaction results remain bound to c301cc213784 and must not be relabeled as full final-seal rerun. Recheck covered the modified paths and basic editing/Paste/Save interactions.
- No actual edit-while-native-modal race, independent QR scanner decode, macOS clipboard/attached-sheet/IME/Spaces/security/release acceptance.
- No screenshots published; all screenshot observations remained local tool inspection. Text receipts and image file artifacts retained for parent.
