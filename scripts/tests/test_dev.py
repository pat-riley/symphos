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
