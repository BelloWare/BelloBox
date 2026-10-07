# Cloud Linux system Open/Save acceptance

This closes the earlier **cloud file-dialog environment gap** for a process-local
launch. It does not enable native movie decoding or capture. The ordinary desktop
session was deliberately left unchanged; launch without the documented portal
session can still report a missing picker.

## Exact app and environment

All interactions used the immutable BelloBox binary
`f002f201725dbd9a08bb2270948a073a7095b2642d7a35ce9a9df4c5f140c970`,
whose source matches published `f403966c4a55dc8af25fb7d725035e7f0cc72faf`
(tree `1a08993a0574eca80c6e48124cce5d19daf91bde`). The 13 modified-source hashes
were verified against that commit; see the existing
[Window source manifest](validation/window-overlay-2026-10-07/source-inputs.json).
The later backend-integration working tree was not used for this evidence.

GPUI 0.2.2's Linux `platform.rs:291–409` calls ashpd's FileChooser portal, not a
direct GTK chooser. An actual desktop D-Bus property probe initially returned
`org.freedesktop.DBus.Error.ServiceUnknown` for `org.freedesktop.portal.Desktop`.
The portal executables were missing. This was an environment prerequisite failure,
not evidence that the application could never present a system dialog.

Only these official packages were downloaded from the configured, signed Debian
trixie snapshot `https://snapshot.debian.org/archive/debian/20260925T181338Z/`:

| Package | Version | SHA-256 |
| --- | --- | --- |
| xdg-desktop-portal | 1.20.3+ds-1 | `9dce4bf7ca1cc9afce6d48735a4a9897a27bcfd17791a05031eb73ac76788922` |
| xdg-desktop-portal-gtk | 1.15.3-1 | `84fdc449f05ffa0772e6eb4c6ccb348a353d2abd382dacc206ba3c5eee71f630` |

Downloaded bytes matched signed APT metadata. `dpkg-deb -x` extracted them into a
separate workspace `portal-sysroot`; no package maintainer scripts, root install,
system configuration, security setting, credential or paid service was used.
Existing GTK/GLib/DBus dependencies resolved with the restored GUI environment.
The download was approximately 536 kB. No package or executable is vendored here.

## Process-local launch recipe

The build shell and cloud desktop share `/workspace`, but not `/tmp`. Keep the
immutable app, package extraction, configuration and launch script in that shared
workspace. Launch through the cloud terminal using CUA, not from the shell's
unconnected display namespace. The retained
[exact inner launch script](validation/cloud-dialogs-2026-10-07/launch-recipe.sh)
uses the known workspace, frozen binary and explicit synthetic Window/GIF routes.
Adapt paths only for a new environment and repeat acceptance before claiming it.

1. Extract both verified packages into `build-environment/portal-sysroot`.
2. Source the existing `gui-env.sh` for software Vulkan, libraries and fonts.
3. Create isolated config/data/cache/export/app-settings directories under
   `build-environment/portal-qa`.
4. Put this `portals.conf` beside `gtk.portal` in the extracted portal directory:

   ```ini
   [preferred]
   default=none
   org.freedesktop.impl.portal.FileChooser=gtk
   ```

   The `XDG_DESKTOP_PORTAL_DIR` override also redirects configuration lookup to
   that directory. An initial probe placed configuration only in XDG_CONFIG_HOME;
   logs showed fallback backend selection. That session was closed, the config
   moved to the observed lookup directory, and the successful matrix below used
   verified `default=none` / FileChooser=gtk selection. No capture/location/camera
   interface was invoked. The frontend has builtin interfaces; this recipe does
   not claim to remove every exported D-Bus interface.
5. Run `dbus-run-session -- bash <inner-script> window` or `gif`. The inner script
   explicitly starts the workspace permission store, GTK backend and frontend,
   probes FileChooser version (observed `uint32 4`), then starts the copied app.
   A trap retires only those child processes after the app closes.

No document-portal FUSE mount or PipeWire capture service was installed or enabled.
The unsandboxed app's native file URI selection worked without them. GTK emitted
a nonfatal missing-service warning; the complete output files and UI results below,
not absence of all warnings, establish this scoped success.

## Actual CUA checks

### Window Save

- Select the app-generated orange Window in the same inline overlay.
- Ctrl+S opens the actual GTK Save dialog:
  [window-save-dialog.jpg](validation/cloud-dialogs-2026-10-07/window-save-dialog.jpg).
- Cancel creates no file and leaves the editor intact. Xfwm returns focus to the
  chooser rather than its notification-style overlay; pointer refocus is required
  before another editor shortcut. No keyboard-only focus restoration is claimed.
- Reopen Save, accept the unique suggested PNG path inside the fixture directory,
  and wait for the real renderer/write worker. The UI reports `Saved PNG.` and
  controls are restored:
  [window-after-save.jpg](validation/cloud-dialogs-2026-10-07/window-after-save.jpg).
- The actual [saved PNG](validation/cloud-dialogs-2026-10-07/saved-window.png) is
  **680×540 RGBA**. Pixel (0,0) is transparent black; pixel (100,100) is the expected
  frozen orange `[255,224,186,255]`. Its hash is retained in the manifest.

### Converter Save and Open

- Launch the existing valid generated movie fixture. Convert to GIF opens the
  actual Save dialog; Cancel preserves source, options and trim and creates no GIF.
- Reopen and choose the valid fixture-directory destination:
  [gif-save-dialog.jpg](validation/cloud-dialogs-2026-10-07/gif-save-dialog.jpg).
- The real exporter saves **45 frames, 320×180, 3,000 ms total, 44,416 bytes**.
  Independent Pillow parsing verifies the file, and the app shows the result with
  its preview paused:
  [gif-after-save.jpg](validation/cloud-dialogs-2026-10-07/gif-after-save.jpg),
  [saved GIF](validation/cloud-dialogs-2026-10-07/saved-synthetic.gif).
- Choose Another opens the system Open dialog. Cancel preserves the current
  source and completed result. Reopen and select the exact generated MP4 path:
  [gif-open-dialog.jpg](validation/cloud-dialogs-2026-10-07/gif-open-dialog.jpg).
- This [input MP4](validation/cloud-dialogs-2026-10-07/synthetic-orange.mp4) was
  generated from an orange color source with the environment's existing FFmpeg;
  ffprobe verifies H.264, 160×90 and one second. No screen or microphone is involved.
- Selection reaches the intended `MovieAsset` boundary and displays native movie
  decoding unavailable:
  [gif-native-gate.jpg](validation/cloud-dialogs-2026-10-07/gif-native-gate.jpg).
  This validates system-dialog routing and honest gate handling, not native movie
  decoding/conversion. Format choices remain; choosing another source resets the
  prior trim/result as designed. The saved GIF file remains unchanged.
- Escape closes the converter. No BelloBox/GTK dialog remains. A new property probe
  on the original desktop bus again returns ServiceUnknown, confirming the private
  portal launch did not register a persistent global desktop service.

All six screenshots are original returned app/dialog-only JPEG bytes, without
transcoding. The [manifest](validation/cloud-dialogs-2026-10-07/manifest.json)
contains exact package/app/source/artifact hashes and dimensions. No Rust source
changed for this restoration; these docs add no production/support Rust LOC.

## Remaining limits

This is actual Linux/X11 interaction with supplied pixels and a generated movie.
It does not establish macOS NSOpenPanel/NSSavePanel behavior, native capture,
AppKit/Spaces/Retina/TCC, original-color fidelity or native MovieAsset decoding.
The normal desktop launch still needs a portal service or this isolated recipe.
No universal absence-of-bugs or application-wide interaction parity is claimed.
