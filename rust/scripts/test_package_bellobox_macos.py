"""Offline packaging policy regression tests; fixtures are never executed."""
import importlib.util
from pathlib import Path
import plistlib
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("preview", Path(__file__).with_name("validate-bellobox-macos.py"))
preview = importlib.util.module_from_spec(spec)
spec.loader.exec_module(preview)
KEY = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="


class PreviewPackagingTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.app = Path(self.temp.name) / "BelloBox Rust.app"
        (self.app / "Contents/MacOS").mkdir(parents=True)
        binary = self.app / "Contents/MacOS/bellobox"
        binary.write_bytes(b"fixture, not an application")
        binary.chmod(0o755)
        self.info = preview.preview_info({})

    def write(self):
        with (self.app / "Contents/Info.plist").open("wb") as handle:
            plistlib.dump(self.info, handle)

    def test_packaging_build_cannot_download_dependencies(self):
        script = Path(__file__).with_name("package-bellobox-macos.sh").read_text()
        builds = [line for line in script.splitlines() if "cargo build" in line]
        self.assertEqual(len(builds), 2)
        self.assertTrue(all("cargo build --offline --locked" in line for line in builds))

    def test_default_is_isolated_offline(self):
        self.assertEqual(self.info["CFBundleIdentifier"], preview.PREVIEW_ID)
        self.assertNotIn("SUFeedURL", self.info)
        self.assertNotIn("SUPublicEDKey", self.info)
        self.write()
        preview.validate_bundle(self.app, require_offline=True)

    def test_feed_requires_explicit_valid_key(self):
        for key in ("", "invalid", "AAAA", "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAB="):
            with self.subTest(key=key), self.assertRaises(ValueError):
                preview.preview_info({"RUST_SPARKLE_FEED_URL": "https://example.com/preview.xml", "SPARKLE_PUBLIC_ED_KEY": key})
        info = preview.preview_info({"RUST_SPARKLE_FEED_URL": "https://example.com/preview.xml", "SPARKLE_PUBLIC_ED_KEY": KEY})
        self.assertEqual(info["SUPublicEDKey"], KEY)

    def test_rejects_unsafe_feeds(self):
        for feed in (preview.PRODUCTION_FEED, preview.PRODUCTION_FEED + "?preview=1", "https://BELLOWARE.com:443/assets/bello_box.appcast.xml", "http://example.com/x", "https://", "https://user@example.com/x", "https://example.com/a b", "https://belloware.com./assets/bello_box.appcast.xml", "https://belloware.com/assets/./bello_box.appcast.xml", "https://belloware.com/assets/%62ello_box.appcast.xml", "https://updates.belloware.com/preview.xml", "https://example.com:443/preview.xml", "https://example.com/preview.xml#fragment", "https://@example.com/preview.xml", "https://example..com/preview.xml", "https://éxample.com/preview.xml"):
            with self.subTest(feed=feed), self.assertRaises(ValueError):
                preview.validate_feed_key(feed, KEY)

    def test_no_orphan_key_or_framework(self):
        for env in ({"SPARKLE_PUBLIC_ED_KEY": KEY}, {"SPARKLE_FRAMEWORK": "/fixture/Sparkle.framework"}):
            with self.assertRaises(ValueError):
                preview.preview_info(env)

    def test_rejects_original_identity_and_defaults_domain(self):
        for change in ({"CFBundleIdentifier": "com.ainoob.BelloBox"}, {"SUDefaultsDomain": "com.ainoob.BelloBox"}):
            self.info = preview.preview_info({}) | change
            self.write()
            with self.assertRaises(ValueError):
                preview.validate_bundle(self.app)

    def test_update_defaults_must_remain_disabled(self):
        for key in ("SUEnableAutomaticChecks", "SUAutomaticallyUpdate", "SUAllowsAutomaticUpdates"):
            self.info = preview.preview_info({}) | {key: True}
            self.write()
            with self.assertRaises(ValueError):
                preview.validate_bundle(self.app)

    def test_offline_rejects_feed_and_framework(self):
        self.info.update(SUFeedURL="https://example.com/preview.xml", SUPublicEDKey=KEY)
        self.write()
        with self.assertRaises(ValueError):
            preview.validate_bundle(self.app, require_offline=True)
        self.info = preview.preview_info({})
        self.write()
        (self.app / "Contents/Frameworks/Sparkle.framework").mkdir(parents=True)
        with self.assertRaises(ValueError):
            preview.validate_bundle(self.app, require_offline=True)

    def test_rejects_missing_or_nonexecutable_binary(self):
        self.write()
        binary = self.app / "Contents/MacOS/bellobox"
        binary.chmod(0o644)
        with self.assertRaises(ValueError):
            preview.validate_bundle(self.app)
        binary.unlink()
        with self.assertRaises(ValueError):
            preview.validate_bundle(self.app)


if __name__ == "__main__":
    unittest.main()
