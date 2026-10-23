import argparse
import importlib.util
from pathlib import Path
import tempfile
import unittest

MODULE = Path(__file__).resolve().parents[1] / "dev.py"
spec = importlib.util.spec_from_file_location("symphos_dev", MODULE)
dev = importlib.util.module_from_spec(spec)
spec.loader.exec_module(dev)


class DevelopmentSupervisorTests(unittest.TestCase):
    def test_cache_budget_counts_only_debug_artifacts_and_ignores_links(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "target/dev").mkdir(parents=True)
            workspace = root / "target/dev/workspace.json"
            workspace.write_bytes(b"session" * 1000)
            self.assertFalse(dev.debug_cache_exceeds(root, budget=1))
            debug = root / "target/debug/deps"
            debug.mkdir(parents=True)
            (debug / "external").symlink_to(workspace)
            self.assertFalse(dev.debug_cache_exceeds(root, budget=1))
            (debug / "compile.rcgu.o").write_bytes(b"object" * 1000)
            self.assertTrue(dev.debug_cache_exceeds(root, budget=1))
            self.assertEqual(workspace.read_bytes(), b"session" * 1000)

    def test_cleanup_preserves_cargo_lock_workspace_release_and_external_data(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name in ["debug", "dev", "release"]:
                (root / "target" / name).mkdir(parents=True)
            debug = root / "target/debug"
            lock = debug / ".cargo-lock"
            lock.touch()
            inode = lock.stat().st_ino
            workspace = root / "target/dev/workspace.json"
            workspace.write_text("session")
            release = root / "target/release/symphos"
            release.write_text("release")
            (debug / "external").symlink_to(workspace)
            (debug / "deps").mkdir()
            (debug / "deps/old.rcgu.o").write_text("old compiler output")
            dev.reclaim_debug_cache(root)
            self.assertEqual(list(debug.iterdir()), [lock])
            self.assertEqual(lock.stat().st_ino, inode)
            self.assertEqual(workspace.read_text(), "session")
            self.assertEqual(release.read_text(), "release")

    def test_source_changes_are_watched_but_live_theme_and_workspace_are_not(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "src").mkdir()
            source = root / "src" / "app.rs"
            source.write_text("first")
            before = dev.fingerprint(root)
            (root / "target/dev").mkdir(parents=True)
            (root / "target/dev/theme.json").write_text("live")
            (root / "target/dev/workspace.json").write_text("session")
            self.assertEqual(before, dev.fingerprint(root))
            source.write_text("second change")
            self.assertNotEqual(before, dev.fingerprint(root))

    def test_gpu_crates_and_shaders_are_watched(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            shader = root / "crates/visuals/src/particles.wgsl"
            shader.parent.mkdir(parents=True)
            shader.write_text("first")
            before = dev.fingerprint(root)
            shader.write_text("second shader")
            self.assertNotEqual(before, dev.fingerprint(root))

    def test_failed_build_leaves_current_app_untouched(self):
        args = argparse.Namespace(build_only=False, system_audio=False, once=False)
        runner = dev.Supervisor(args)
        runner.app = object()
        previous = runner.app
        runner.build = lambda: None
        runner.publish = lambda _built: self.fail("failed build must not publish or stop app")
        self.assertFalse(runner.rebuild())
        self.assertIs(previous, runner.app)

    def test_completed_build_is_published_only_after_success(self):
        args = argparse.Namespace(build_only=False, system_audio=False, once=False)
        runner = dev.Supervisor(args)
        built = ("staging", "destination", 1.5)
        runner.build = lambda: built
        published = []
        runner.publish = published.append
        self.assertTrue(runner.rebuild())
        self.assertEqual([built], published)

    def test_control_files_are_atomically_replaced(self):
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "control.json"
            dev.atomic_json(target, {"action": "close", "request": 1})
            dev.atomic_json(target, {"action": "close", "request": 2})
            self.assertEqual(dev.json.loads(target.read_text())["request"], 2)
            self.assertFalse(target.with_suffix(".tmp").exists())


if __name__ == "__main__":
    unittest.main()
