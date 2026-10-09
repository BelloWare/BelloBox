#!/usr/bin/env python3
"""Offline preview metadata/assembly checks; never load code or contact a feed."""
import argparse
import base64
import binascii
import os
from pathlib import Path
import plistlib

PREVIEW_ID = "com.ainoob.BelloBox.rust.preview"
NOTICE_FILES = ("swift-comparison-attribution.md", "licenses/swift-runtime-LICENSE.txt")
SOURCE_NOTICES = Path(__file__).resolve().parent.parent / "docs"
PRODUCTION_FEED = "https://belloware.com/assets/bello_box.appcast.xml"


def validate_feed_key(feed, key):
    # Intentionally narrow preview URL grammar, mirrored by the native preflight.
    # Reject ports/escaped authorities/Unicode rather than guessing equivalence.
    if (not feed.startswith("https://") or not feed.isascii()
            or any(c.isspace() or ord(c) < 32 or ord(c) == 127 for c in feed)
            or any(c in feed for c in ("@", "#", "%", "\\"))):
        raise ValueError("Preview feed must be unambiguous ASCII HTTPS without credentials, fragments or escapes")
    host = feed[8:].split("/", 1)[0].split("?", 1)[0]
    labels = host.split(".")
    if (len(host) > 253 or any(not label or len(label) > 63
            or not label[0].isalnum() or not label[-1].isalnum()
            or any(not (c.isalnum() or c == "-") for c in label)
            for label in labels)):
        raise ValueError("Preview feed requires a plain DNS hostname without a port")
    if host.lower() == "belloware.com" or host.lower().endswith(".belloware.com"):
        raise ValueError("Production Belloware hosts cannot serve Rust preview updates")
    try:
        decoded = base64.b64decode(key, validate=True)
    except (ValueError, binascii.Error):
        decoded = b""
    if len(decoded) != 32 or base64.b64encode(decoded).decode("ascii") != key:
        raise ValueError("An explicit 32-byte base64 SPARKLE_PUBLIC_ED_KEY is required")


def preview_info(env):
    info = {
        "CFBundleIdentifier": PREVIEW_ID, "CFBundleName": "BelloBox Rust",
        "CFBundleDisplayName": "BelloBox Rust", "CFBundleExecutable": "bellobox",
        "CFBundlePackageType": "APPL",
        "CFBundleShortVersionString": env.get("APP_VERSION", "0.1.0"),
        "CFBundleVersion": env.get("APP_BUILD", "1"), "LSMinimumSystemVersion": "13.0",
        "NSPrincipalClass": "NSApplication", "LSUIElement": False,
        "NSHighResolutionCapable": True, "SUEnableAutomaticChecks": False,
        "SUAutomaticallyUpdate": False, "SUAllowsAutomaticUpdates": False,
        "NSHumanReadableCopyright": "Copyright © 2026 Belloware. All rights reserved.",
    }
    feed = env.get("RUST_SPARKLE_FEED_URL", "")
    key = env.get("SPARKLE_PUBLIC_ED_KEY", "")
    if feed:
        validate_feed_key(feed, key)
        info.update(SUFeedURL=feed, SUPublicEDKey=key)
    elif key or env.get("SPARKLE_FRAMEWORK"):
        raise ValueError("Sparkle key/framework requires an explicit preview feed")
    return info


def validate_bundle(app, require_offline=False):
    app = Path(app)
    with (app / "Contents/Info.plist").open("rb") as handle:
        info = plistlib.load(handle)
    if info.get("CFBundleIdentifier") != PREVIEW_ID:
        raise ValueError("Only the isolated Rust preview bundle identity is allowed")
    for key in ("SUEnableAutomaticChecks", "SUAutomaticallyUpdate", "SUAllowsAutomaticUpdates"):
        if info.get(key) is not False:
            raise ValueError(f"{key} must be false for a preview")
    if "SUDefaultsDomain" in info:
        raise ValueError("Preview must use its own bundle preferences domain")
    if info.get("CFBundleExecutable") != "bellobox" or info.get("CFBundlePackageType") != "APPL":
        raise ValueError("Invalid preview executable/package metadata")
    executable = app / "Contents/MacOS/bellobox"
    if executable.is_symlink() or not executable.is_file() or not os.access(executable, os.X_OK):
        raise ValueError("Preview executable must be a regular executable file")
    notices = app / "Contents/Resources/ThirdPartyNotices"
    for directory in (notices, notices / "licenses"):
        if directory.is_symlink() or not directory.is_dir():
            raise ValueError("Preview must include regular third-party notice directories")
    for relative in NOTICE_FILES:
        bundled = notices / relative
        source = SOURCE_NOTICES / relative
        if bundled.is_symlink() or not bundled.is_file():
            raise ValueError(f"Missing regular third-party notice: {relative}")
        if bundled.read_bytes() != source.read_bytes():
            raise ValueError(f"Third-party notice differs from reviewed source: {relative}")
    framework = app / "Contents/Frameworks/Sparkle.framework"
    feed = info.get("SUFeedURL")
    if require_offline and (feed is not None or "SUPublicEDKey" in info or framework.exists() or framework.is_symlink()):
        raise ValueError("Offline preview must have no update feed, key or framework")
    if feed is not None:
        validate_feed_key(feed, info.get("SUPublicEDKey", ""))
    elif "SUPublicEDKey" in info or framework.exists() or framework.is_symlink():
        raise ValueError("Sparkle configuration requires an explicit preview feed")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("app", type=Path)
    parser.add_argument("--write-info", action="store_true")
    parser.add_argument("--require-offline", action="store_true")
    args = parser.parse_args()
    if args.write_info:
        info = preview_info(os.environ)
        with (args.app / "Contents/Info.plist").open("xb") as handle:
            plistlib.dump(info, handle)
    else:
        validate_bundle(args.app, args.require_offline)
        print("Preview assembly validated; runtime, update trust and distribution not tested.")


if __name__ == "__main__":
    main()
