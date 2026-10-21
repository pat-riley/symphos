#!/usr/bin/env python3
"""Build on save, retain the last good app, and supervise only our dev process."""
import argparse
import fcntl
import json
import os
from pathlib import Path
import plistlib
import shutil
import signal
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parent.parent
DEV = ROOT / "target" / "dev"


def atomic_json(path, value):
    temporary = path.with_suffix(".tmp")
    temporary.write_text(json.dumps(value))
    temporary.replace(path)


def fingerprint(root=ROOT):
    files = [root / name for name in ("Cargo.toml", "Cargo.lock", "build.rs", "rust-toolchain.toml")]
    for name in ("src", "crates", "vendor/projectm", "assets/milkdrop", "assets/macos", "assets/development"):
        directory = root / name
        if directory.exists():
            files.extend(path for path in directory.rglob("*") if path.is_file())
    return tuple(sorted((str(path.relative_to(root)), path.stat().st_mtime_ns, path.stat().st_size)
                        for path in files if path.exists()))


class Supervisor:
    def __init__(self, args):
        self.args = args
        self.app = None
        self.stopping = False
        self.dev = DEV
        self.env = os.environ.copy()
        self.env["SYMPHOS_DEV_DIR"] = str(self.dev)
        self.env["MACOSX_DEPLOYMENT_TARGET"] = "14.2"
        self.env.setdefault("RUST_LOG", "symphos=info,wgpu_core=warn")
        self.app_log = None

    def status(self, message):
        atomic_json(self.dev / "status.json", {"message": message, "updated": time.time()})
        print(f"[Symphos Dev] {message}", flush=True)

    def stop_app(self):
        if self.app is None:
            return
        if self.app.poll() is None:
            atomic_json(self.dev / "control.json", {"action": "close", "request": time.time_ns()})
            try:
                self.app.wait(timeout=5)
            except subprocess.TimeoutExpired:
                # This PID is a child we created, never a user's normal app.
                self.app.terminate()
                try:
                    self.app.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    self.app.kill()
                    self.app.wait()
        self.app = None
        if self.app_log is not None:
            self.app_log.close()
            self.app_log = None

    def build(self):
        self.status("Building...")
        started = time.monotonic()
        with (self.dev / "build.log").open("w") as output:
            process = subprocess.Popen(["cargo", "build", "--features", "development", "--locked"],
                                       cwd=ROOT, env=self.env, stdout=subprocess.PIPE,
                                       stderr=subprocess.STDOUT, text=True, errors="replace")
            for line in process.stdout:
                print(line, end="", flush=True)
                output.write(line)
                output.flush()
            result = process.wait()
        if result != 0:
            self.status("Build failed; current app kept")
            return None
        elapsed = time.monotonic() - started
        # Package/sign a staging bundle before stopping the working app.
        if sys.platform == "darwin":
            staging = self.dev / "staging" / "Symphos Dev.app"
            if staging.exists():
                shutil.rmtree(staging)
            executable = staging / "Contents" / "MacOS" / "symphos"
            executable.parent.mkdir(parents=True)
            resources = staging / "Contents" / "Resources"
            resources.mkdir()
            shutil.copy2(ROOT / "THIRD_PARTY_NOTICES.md", resources)
            licenses = resources / "Licenses"
            licenses.mkdir()
            for notice in (ROOT / "vendor/projectm/licenses").glob("*.txt"):
                shutil.copy2(notice, licenses)
            for name in ("COPYING", "LICENSE.txt"):
                shutil.copy2(ROOT / "vendor/projectm" / name, licenses)
            shutil.copy2(ROOT / "target/debug/symphos", executable)
            with (ROOT / "assets/macos/Info.plist").open("rb") as source:
                info = plistlib.load(source)
            info.update(CFBundleIdentifier="io.github.riley.Symphos.Dev", CFBundleName="Symphos Dev",
                        CFBundleDisplayName="Symphos Dev")
            with (staging / "Contents" / "Info.plist").open("wb") as target:
                plistlib.dump(info, target)
            identity = os.environ.get("SYMPHOS_SIGN_IDENTITY", "-")
            subprocess.run(["codesign", "--force", "--sign", identity, str(staging)], check=True)
            subprocess.run(["codesign", "--verify", "--deep", "--strict", str(staging)], check=True)
            destination = self.dev / "Symphos Dev.app"
        else:
            staging = self.dev / "symphos.next"
            shutil.copy2(ROOT / "target/debug/symphos", staging)
            destination = self.dev / "symphos"
        return staging, destination, elapsed

    def publish(self, built):
        staging, destination, elapsed = built
        self.stop_app()
        if destination.is_dir():
            previous = self.dev / "previous.app"
            if previous.exists():
                shutil.rmtree(previous)
            destination.rename(previous)
            staging.rename(destination)
            shutil.rmtree(previous)
        else:
            staging.replace(destination)
        executable = destination / "Contents/MacOS/symphos" if sys.platform == "darwin" else destination
        if self.args.build_only:
            self.status(f"Built in {elapsed:.1f}s: {destination}")
            return
        self.app_log = (self.dev / "app.log").open("a")
        command = [str(executable), "--dev"]
        if self.args.system_audio:
            command.append("--system-audio")
        elif self.args.demo_audio:
            command.append("--demo-audio")
        self.app = subprocess.Popen(command, cwd=ROOT, env=self.env, stdout=self.app_log, stderr=self.app_log)
        self.status(f"Ready · build {elapsed:.1f}s")

    def rebuild(self):
        try:
            built = self.build()
            if built is not None and not self.stopping:
                self.publish(built)
            return built is not None
        except (OSError, subprocess.CalledProcessError) as error:
            self.status(f"Build/package failed: {error}")
            return False

    def run(self):
        previous = fingerprint()
        success = self.rebuild()
        if self.args.build_only:
            return 0 if success else 1
        print(f"[Symphos Dev] Edit Rust to rebuild. Edit {self.dev / 'theme.json'} for live colors/scale.")
        print(f"[Symphos Dev] Logs: {self.dev}. Ctrl+C stops this dev app.")
        pending = None
        changed_at = None
        while not self.stopping:
            if self.args.once:
                if self.app is None or self.app.poll() is not None:
                    break
            else:
                current = fingerprint()
                if current != previous:
                    if current != pending:
                        pending = current
                        changed_at = time.monotonic()
                    elif time.monotonic() - changed_at >= 0.35:
                        previous = pending
                        pending = None
                        self.rebuild()
                else:
                    pending = None
            time.sleep(0.2)
        return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="Run formatting, lint, and tests for normal and development builds")
    audio = parser.add_mutually_exclusive_group()
    audio.add_argument("--system-audio", action="store_true", help="Start with real playback on every rebuild")
    audio.add_argument("--demo-audio", action="store_true", help="Start with generated audio on every rebuild; default restores the saved mode")
    parser.add_argument("--build-only", action="store_true", help="Build/package the development app without launching or watching")
    parser.add_argument("--once", action="store_true", help="Build and run once without watching")
    args = parser.parse_args()
    if not shutil.which("cargo"):
        parser.error("Cargo is not on PATH. Install Rust or use scripts/dev.sh.")
    if args.check:
        env = os.environ.copy()
        if sys.platform == "darwin":
            env["MACOSX_DEPLOYMENT_TARGET"] = "14.2"
        commands = [
            ["cargo", "fmt", "--all", "--", "--check"],
            ["cargo", "clippy", "--all-targets", "--locked", "--", "-D", "warnings"],
            ["cargo", "clippy", "--all-targets", "--features", "development", "--locked", "--", "-D", "warnings"],
            ["cargo", "test", "--all-targets", "--locked", "--quiet"],
            ["cargo", "test", "--all-targets", "--features", "development", "--locked", "--quiet"],
            [sys.executable, "-m", "unittest", "discover", "-s", "scripts/tests"],
        ]
        for command in commands:
            print("[Symphos Dev] " + " ".join(command), flush=True)
            result = subprocess.run(command, cwd=ROOT, env=env).returncode
            if result:
                return result
        return 0
    DEV.mkdir(parents=True, exist_ok=True)
    lock = (DEV / "supervisor.lock").open("w")
    try:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        parser.error("A development supervisor is already running for this checkout.")
    supervisor = Supervisor(args)
    def stop(_signal, _frame):
        supervisor.stopping = True
    signal.signal(signal.SIGINT, stop)
    signal.signal(signal.SIGTERM, stop)
    try:
        return supervisor.run()
    finally:
        supervisor.stop_app()
        supervisor.status("Stopped" if not args.build_only else "Build command finished")
        lock.close()


if __name__ == "__main__":
    sys.exit(main())
