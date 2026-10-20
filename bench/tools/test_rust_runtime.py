"""Prove that pinned Rust executables are retained, checked, selected and rolled back without touching pinned operations.

Most tests use shell scripts as executables in disposable stores. ``RealBuilds`` builds a small crate with the active
cargo in a disposable repository; it is skipped without cargo unless ``RUST_RUNTIME_REQUIRE_CARGO=1``.
"""
from __future__ import annotations

from contextlib import redirect_stderr, redirect_stdout
import fcntl
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

import rust_runtime

TOOL = Path(__file__).resolve().with_name("rust_runtime.py")
DEBUG_ASSERTIONS = "[profile.release]\ndebug-assertions = true\n"


def fake_receipt(executable: Path, name: str = "fixture-tool", **changes) -> dict:
    receipt = {
        "schema": rust_runtime.RECEIPT,
        "crate": {"name": name, "version": "0.1.0", "path": f"rust/{name}", "bin": name},
        "source": {"repository": None, "revision": "a" * 40, "tree": "b" * 40, "cargo_toml_sha256": "c" * 64,
                   "cargo_lock_sha256": "d" * 64, "toolchain_file_sha256": "e" * 64},
        "toolchain": {"channel": "1.99.1", "rustc": {"release": "1.99.1", "host": rust_runtime.host_target()},
                      "cargo": "cargo 1.99.1"},
        "target": rust_runtime.host_target(), "libc": rust_runtime.host_libc(),
        "build": {"command": ["cargo", "build"], "rustflags": [], "environment": [], "cargo_config_files": []},
        "artifact": {"file": name, "sha256": rust_runtime.sha256_file(executable), "size": executable.stat().st_size},
        "built_at": "2026-10-18T00:00:00Z",
    }
    for key, value in changes.items():
        receipt[key] = value
    return receipt


def script(path: Path, output: str) -> Path:
    path.write_text(f"#!/bin/sh\necho {output}\n", encoding="utf-8")
    path.chmod(0o755)
    return path


class Store(unittest.TestCase):
    def setUp(self):
        self.root = Path(self.enterContext(tempfile.TemporaryDirectory(prefix="rust-runtime-test-"))).resolve()
        self.store = self.root / "store"

    def version(self, output: str, name: str = "fixture-tool", **changes) -> dict:
        executable = script(self.root / f"{name}-{output}", output)
        directory, added = rust_runtime.retain(self.store, executable, fake_receipt(executable, name, **changes))
        self.assertTrue(added)
        return rust_runtime.pin_of(directory)

    def entry(self, pin: dict, name: str = "fixture-tool") -> Path:
        return self.store / name / pin["sha256"]

    def unlock(self, path: Path) -> None:
        path.chmod(0o755)

    def cli(self, *args: str, store: Path | None = None) -> tuple:
        with redirect_stdout(io.StringIO()) as out, redirect_stderr(io.StringIO()) as err:
            code = rust_runtime.main([*args, "--store", str(store or self.store)])
        return code, out.getvalue(), err.getvalue()


class Checks(Store):
    def test_a_linux_arm64_host_refuses_a_retained_linux_arm64_binary(self):
        pin = self.version("linux-arm64", target="aarch64-unknown-linux-gnu")
        with patch.object(rust_runtime.platform, "system", return_value="Linux"), \
                patch.object(rust_runtime.platform, "machine", return_value="aarch64"):
            self.assertIsNone(rust_runtime.host_target())
            code, out, err = self.cli("verify", "fixture-tool", pin["sha256"])
        self.assertEqual((code, out), (1, ""))
        self.assertIn("built for aarch64-unknown-linux-gnu; this host is Linux aarch64", err)

    def test_a_retained_version_is_read_only_and_runs_from_its_entry(self):
        pin = self.version("one")
        executable, receipt = rust_runtime.check(self.store, "fixture-tool", pin)
        self.assertEqual(subprocess.check_output([executable], text=True), "one\n")
        self.assertEqual(receipt["artifact"]["sha256"], pin["sha256"])
        self.assertFalse(os.stat(executable).st_mode & 0o222)
        self.assertFalse(os.stat(executable.with_name("receipt.json")).st_mode & 0o222)

    def test_a_missing_replaced_or_unexecutable_binary_is_refused(self):
        pin = self.version("one")
        executable = self.entry(pin) / "fixture-tool"
        original = executable.read_bytes()

        def replace(text: str) -> None:
            executable.unlink()
            executable.write_text(text, encoding="utf-8")

        cases = [("is missing", executable.unlink),
                 ("no longer hashes to its pin", lambda: replace("#!/bin/sh\necho two\n")),
                 ("is not executable", lambda: executable.chmod(0o444)),
                 ("is missing", lambda: (executable.unlink(), executable.symlink_to(self.root / "fixture-tool-one")))]
        for needle, change in cases:
            with self.subTest(needle=needle):
                change()
                with self.assertRaisesRegex(rust_runtime.Refused, needle):
                    rust_runtime.check(self.store, "fixture-tool", pin)
                executable.unlink(missing_ok=True)
                executable.write_bytes(original)
                executable.chmod(0o555)
                rust_runtime.check(self.store, "fixture-tool", pin)

    def test_a_changed_or_incomplete_receipt_is_refused(self):
        pin = self.version("one")
        receipt = self.entry(pin) / "receipt.json"
        self.unlock(receipt)
        saved = json.loads(receipt.read_text(encoding="utf-8"))
        receipt.write_text(json.dumps({**saved, "built_at": "later"}), encoding="utf-8")
        with self.assertRaisesRegex(rust_runtime.Refused, "receipt.json no longer hashes to its pin"):
            rust_runtime.check(self.store, "fixture-tool", pin)
        incomplete = {key: value for key, value in saved.items() if key != "toolchain"}
        receipt.write_text(json.dumps(incomplete), encoding="utf-8")
        with self.assertRaisesRegex(rust_runtime.Refused, "a field is missing"):
            rust_runtime.check(self.store, "fixture-tool", {**pin, "receipt_sha256": rust_runtime.sha256_file(receipt)})
        receipt.unlink()
        with self.assertRaisesRegex(rust_runtime.Refused, "receipt.json is missing"):
            rust_runtime.check(self.store, "fixture-tool", pin)

    def test_a_binary_for_another_target_libc_or_an_unsupported_host_is_refused(self):
        foreign = next(target for target in sorted(rust_runtime.SUPPORTED_TARGETS) if target != rust_runtime.host_target())
        other = self.version("foreign", target=foreign)
        with self.assertRaisesRegex(rust_runtime.Refused, f"built for {foreign}; this host is"):
            rust_runtime.check(self.store, "fixture-tool", other)
        newer = self.version("newer-libc", libc={"name": "glibc", "version": "99.0"})
        with self.assertRaisesRegex(rust_runtime.Refused, "links glibc 99.0; this host has"):
            rust_runtime.check(self.store, "fixture-tool", newer)
        musl = self.version("musl", libc={"name": "musl", "version": "1.2"})
        with self.assertRaisesRegex(rust_runtime.Refused, "links musl 1.2"):
            rust_runtime.check(self.store, "fixture-tool", musl)
        here = self.version("here")
        with patch.object(rust_runtime, "SUPPORTED_TARGETS", rust_runtime.SUPPORTED_TARGETS - {rust_runtime.host_target()}):
            with self.assertRaisesRegex(rust_runtime.Refused, f"{rust_runtime.host_target()} is not a supported Rust runtime target"):
                rust_runtime.check(self.store, "fixture-tool", here)
        with patch.object(rust_runtime, "host_target", return_value=None):
            with self.assertRaisesRegex(rust_runtime.Refused, "this host is"):
                rust_runtime.check(self.store, "fixture-tool", here)

    def test_an_entry_is_never_replaced_and_a_wrong_hash_is_never_retained(self):
        pin = self.version("one")
        executable = self.root / "fixture-tool-one"
        directory, added = rust_runtime.retain(self.store, executable, fake_receipt(executable, built_at="again"))
        self.assertEqual((directory, added), (self.entry(pin), False))
        other_source = fake_receipt(executable)
        other_source["source"] = {**other_source["source"], "tree": "f" * 40}
        with self.assertRaisesRegex(rust_runtime.Refused, "already retains this executable with another receipt"):
            rust_runtime.retain(self.store, executable, other_source)
        lying = fake_receipt(executable)
        script(executable, "changed")
        with self.assertRaisesRegex(rust_runtime.Refused, "does not hash to the receipt"):
            rust_runtime.retain(self.store, executable, lying)
        self.assertEqual(rust_runtime.pin_of(self.entry(pin)), pin)
        self.assertEqual([path.name for path in (self.store / "fixture-tool").iterdir() if path.name.startswith(".partial")], [])

    def test_a_receipt_with_an_absolute_or_escaping_path_is_not_portable(self):
        executable = script(self.root / "tool", "one")
        for crate_path in ("/home/someone/rust/tool", "../outside"):
            receipt = fake_receipt(executable)
            receipt["crate"] = {**receipt["crate"], "path": crate_path}
            self.assertIn("repository-relative path", rust_runtime.receipt_problem(receipt))
        receipt = fake_receipt(executable)
        receipt["artifact"] = {**receipt["artifact"], "file": "../tool"}
        self.assertIn("file name within its entry", rust_runtime.receipt_problem(receipt))

    def test_restore_retains_a_copied_entry_in_another_store_only_when_it_matches(self):
        pin = self.version("one")
        moved = self.root / "moved"
        shutil.copytree(self.entry(pin), moved)
        other = self.root / "other-store"
        code, out, err = self.cli("restore", "--receipt", str(moved / "receipt.json"), "--executable", str(moved / "fixture-tool"),
                                  store=other)
        self.assertEqual(code, 0, err)
        self.assertEqual(json.loads(out)["pin"], pin)
        executable, _ = rust_runtime.check(other, "fixture-tool", pin)
        self.assertEqual(subprocess.check_output([executable], text=True), "one\n")
        tampered = self.root / "tampered"
        script(tampered, "two")
        code, out, err = self.cli("restore", "--receipt", str(moved / "receipt.json"), "--executable", str(tampered),
                                  store=self.root / "third")
        self.assertEqual((code, out), (1, ""))
        self.assertIn("does not hash to the receipt", err)


class Selection(Store):
    def test_rollback_changes_only_the_selection_new_operations_read(self):
        one, two = self.version("one"), self.version("two")
        rust_runtime.select(self.store, "fixture-tool", one)
        rust_runtime.select(self.store, "fixture-tool", two)
        started = rust_runtime.current_pin(self.store, "fixture-tool")
        self.assertEqual(started, two)
        code, out, err = self.cli("rollback", "fixture-tool")
        self.assertEqual(code, 0, err)
        self.assertEqual(json.loads(out)["action"], "rollback")
        self.assertEqual(rust_runtime.current_pin(self.store, "fixture-tool"), one)
        executable, _ = rust_runtime.check(self.store, "fixture-tool", started)
        self.assertEqual(subprocess.check_output([executable], text=True), "two\n")
        self.assertEqual(sorted(path.name for path in (self.store / "fixture-tool").iterdir() if len(path.name) == 64),
                         sorted([one["sha256"], two["sha256"]]))
        history = [json.loads(line) for line in (self.store / "fixture-tool/selections.jsonl").read_text(encoding="utf-8").splitlines()]
        self.assertEqual([(row["action"], row["pin"]["sha256"]) for row in history],
                         [("select", one["sha256"]), ("select", two["sha256"]), ("rollback", one["sha256"])])
        code, out, err = self.cli("pin", "fixture-tool")
        self.assertEqual(json.loads(out), {"fixture-tool": one})

    def test_rollback_needs_a_retained_previous_version(self):
        one = self.version("one")
        code, _, err = self.cli("rollback", "fixture-tool")
        self.assertEqual(code, 1)
        self.assertIn("no previously selected version", err)
        rust_runtime.select(self.store, "fixture-tool", one)
        two = self.version("two")
        rust_runtime.select(self.store, "fixture-tool", two)
        receipt = self.entry(one) / "receipt.json"
        saved = receipt.read_bytes()
        self.unlock(receipt)
        receipt.write_bytes(saved.replace(b'"built_at"', b'"built_at" ', 1))
        code, _, err = self.cli("rollback", "fixture-tool")
        self.assertEqual(code, 1)
        self.assertIn("receipt.json no longer hashes to its pin", err)
        receipt.write_bytes(saved)
        (self.entry(one) / "fixture-tool").unlink()
        code, _, err = self.cli("rollback", "fixture-tool")
        self.assertEqual(code, 1)
        self.assertIn("executable", err)
        self.assertEqual(rust_runtime.current_pin(self.store, "fixture-tool"), two)

    def test_a_changed_selected_version_is_not_pinned_for_a_new_operation(self):
        one = self.version("one")
        rust_runtime.select(self.store, "fixture-tool", one)
        executable = self.entry(one) / "fixture-tool"
        self.unlock(executable)
        executable.write_text("#!/bin/sh\necho swapped\n", encoding="utf-8")
        code, out, err = self.cli("pin", "fixture-tool")
        self.assertEqual((code, out), (1, ""))
        self.assertIn("no longer hashes to its pin", err)

    def test_a_rollback_returns_to_the_version_its_selection_replaced_despite_a_concurrent_select(self):
        a, b, c = self.version("a"), self.version("b"), self.version("c")
        rust_runtime.select(self.store, "fixture-tool", a)
        rust_runtime.select(self.store, "fixture-tool", b)
        real, racer = rust_runtime.selected, []

        def selected_then_race(store, name):
            current = real(store, name)
            if not racer:
                racer.append(subprocess.Popen([sys.executable, str(TOOL), "select", "fixture-tool", c["sha256"], "--store", str(self.store)],
                                              stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True))
                time.sleep(0.5)
            return current

        with patch.object(rust_runtime, "selected", selected_then_race):
            rust_runtime.rollback(self.store, "fixture-tool")
        self.assertEqual(racer[0].wait(60), 0, racer[0].stderr.read())
        racer[0].stderr.close()
        rows = [json.loads(line) for line in (self.store / "fixture-tool/selections.jsonl").read_text(encoding="utf-8").splitlines()]
        self.assertEqual([(row["action"], row["pin"]) for row in rows], [("select", a), ("select", b), ("rollback", a), ("select", c)])
        for earlier, later in zip(rows, rows[1:]):
            self.assertEqual(later["previous"], earlier["pin"])
            if later["action"] == "rollback":
                self.assertEqual(later["pin"], earlier["previous"])
        self.assertEqual(rust_runtime.current_pin(self.store, "fixture-tool"), c)

    def test_concurrent_writers_serialize_on_the_store_lock(self):
        pins = [self.version(str(index)) for index in range(4)]
        writers = [subprocess.Popen([sys.executable, str(TOOL), "select", "fixture-tool", pins[index % 4]["sha256"],
                                     "--store", str(self.store)], stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True)
                   for index in range(16)]
        self.assertEqual([writer.wait(60) for writer in writers], [0] * 16, [writer.stderr.read() for writer in writers])
        for writer in writers:
            writer.stderr.close()
        rows = [json.loads(line) for line in (self.store / "fixture-tool/selections.jsonl").read_text(encoding="utf-8").splitlines()]
        for earlier, later in zip(rows, rows[1:]):
            self.assertEqual(later["previous"], earlier["pin"])
        self.assertEqual(rust_runtime.selected(self.store, "fixture-tool"), rows[-1])


class Resolution(Store):
    """Which executable an operation runs, without a fallback to another version or to Python."""

    def setUp(self):
        super().setUp()
        self.repo = self.root / "repo"
        (self.repo / "rust/fixture-tool").mkdir(parents=True)
        self.git("init", "-q")
        self.commit("one")
        self.enterContext(patch.dict(os.environ))
        for name in ("BENCH_RUST_FIXTURE_TOOL", "BENCH_RUST_FIXTURE_TOOL_SHA256", rust_runtime.STORE_VARIABLE):
            os.environ.pop(name, None)

    def git(self, *args: str) -> str:
        return subprocess.check_output(["git", "-C", str(self.repo), "-c", "user.name=Fixture",
                                        "-c", "user.email=fixture@example.invalid", *args], text=True).strip()

    def commit(self, text: str) -> None:
        (self.repo / "rust/fixture-tool/main.rs").write_text(text, encoding="utf-8")
        self.git("add", ".")
        self.git("commit", "-qm", text)

    def head_version(self, output: str) -> dict:
        tree = self.git("rev-parse", "HEAD:rust/fixture-tool")
        source = {**fake_receipt(script(self.root / "probe", "x"))["source"], "tree": tree}
        return self.version(output, source=source)

    def resolve(self, run_dir: Path | None = None) -> Path:
        return rust_runtime.resolve("fixture-tool", run_dir, self.store, self.repo)

    def test_the_selected_version_runs_only_while_its_crate_tree_is_the_one_at_head(self):
        with self.assertRaisesRegex(rust_runtime.Refused, "no selected version.*install it with: python3 "
                                    "bench/tools/rust_runtime.py install --crate rust/fixture-tool --select"):
            self.resolve()
        pin = self.head_version("one")
        rust_runtime.select(self.store, "fixture-tool", pin)
        self.assertEqual(self.resolve(), self.entry(pin) / "fixture-tool")
        self.commit("two")
        with self.assertRaisesRegex(rust_runtime.Refused, "selected fixture-tool was built from crate tree .*; "
                                    "HEAD has .*; install it with"):
            self.resolve()

    def test_a_launch_variable_is_used_only_with_its_matching_hash(self):
        rust_runtime.select(self.store, "fixture-tool", self.head_version("selected"))
        launched = script(self.root / "launched", "launched")
        os.environ["BENCH_RUST_FIXTURE_TOOL"] = str(launched)
        with self.assertRaisesRegex(rust_runtime.Refused, "must be set together"):
            self.resolve()
        os.environ["BENCH_RUST_FIXTURE_TOOL_SHA256"] = "0" * 64
        with self.assertRaisesRegex(rust_runtime.Refused, "does not hash to BENCH_RUST_FIXTURE_TOOL_SHA256"):
            self.resolve()
        os.environ["BENCH_RUST_FIXTURE_TOOL_SHA256"] = rust_runtime.sha256_file(launched)
        self.assertEqual(self.resolve(), launched)
        os.environ["BENCH_RUST_FIXTURE_TOOL"] = "launched"
        with self.assertRaisesRegex(rust_runtime.Refused, "not the absolute path"):
            self.resolve()
        self.assertEqual(rust_runtime.variable("fixture-tool"), "BENCH_RUST_FIXTURE_TOOL")
        import review_queue
        self.assertEqual(rust_runtime.variable("bench-tools"), review_queue.runtime_variable("bench-tools"))

    def test_a_run_uses_its_frozen_pin_and_never_the_selection(self):
        frozen, selected = self.version("frozen"), self.head_version("selected")
        rust_runtime.select(self.store, "fixture-tool", selected)
        run_dir = self.root / "run"
        run_dir.mkdir()
        with self.assertRaisesRegex(rust_runtime.Refused, "runtimes.frozen.json is missing"):
            self.resolve(run_dir)
        (run_dir / "runtimes.frozen.json").write_text(json.dumps({"other-tool": selected}), encoding="utf-8")
        with self.assertRaisesRegex(rust_runtime.Refused, "pins no fixture-tool runtime"):
            self.resolve(run_dir)
        (run_dir / "runtimes.frozen.json").write_text(json.dumps({"fixture-tool": frozen}), encoding="utf-8")
        self.assertEqual(self.resolve(run_dir), self.entry(frozen) / "fixture-tool")
        executable = self.entry(frozen) / "fixture-tool"
        self.unlock(executable)
        executable.write_text("#!/bin/sh\necho changed\n", encoding="utf-8")
        with self.assertRaisesRegex(rust_runtime.Refused, "no longer hashes to its pin"):
            self.resolve(run_dir)

    def test_a_run_operation_needs_the_run_to_pin_the_executable_it_runs(self):
        frozen, selected = self.version("frozen"), self.head_version("selected")
        rust_runtime.select(self.store, "fixture-tool", selected)
        run_dir = self.root / "2026-01-01-frozen-before"
        run_dir.mkdir()
        (run_dir / "manifest.json").write_text(json.dumps({"freeze_commit": "f" * 40}), encoding="utf-8")
        launched = script(self.root / "launched", "launched")
        os.environ.update(BENCH_RUST_FIXTURE_TOOL=str(launched), BENCH_RUST_FIXTURE_TOOL_SHA256=rust_runtime.sha256_file(launched))
        unpinned = f"run 2026-01-01-frozen-before pins no fixture-tool runtime; run it from a checkout of its freeze commit {'f' * 40}"
        for pins in (None, {"other-tool": selected}):
            if pins is not None:
                (run_dir / "runtimes.frozen.json").write_text(json.dumps(pins), encoding="utf-8")
            with self.assertRaisesRegex(rust_runtime.Refused, unpinned):
                rust_runtime.for_run("fixture-tool", run_dir, self.store)
        (run_dir / "runtimes.frozen.json").write_text(json.dumps({"fixture-tool": frozen}), encoding="utf-8")
        with self.assertRaisesRegex(rust_runtime.Refused, f"{launched} is not the fixture-tool that .*runtimes.frozen.json pins"):
            rust_runtime.for_run("fixture-tool", run_dir, self.store)
        for name in ("BENCH_RUST_FIXTURE_TOOL", "BENCH_RUST_FIXTURE_TOOL_SHA256"):
            os.environ.pop(name)
        self.assertEqual(rust_runtime.for_run("fixture-tool", run_dir, self.store), self.entry(frozen) / "fixture-tool")
        launched = self.entry(frozen) / "fixture-tool"
        os.environ.update(BENCH_RUST_FIXTURE_TOOL=str(launched), BENCH_RUST_FIXTURE_TOOL_SHA256=frozen["sha256"])
        self.assertEqual(rust_runtime.for_run("fixture-tool", run_dir, self.store), launched)

    def test_exec_runs_the_resolved_executable_with_its_arguments_and_exit_code(self):
        program = self.root / "echo-args"
        program.write_text('#!/bin/sh\nprintf "%s|" "$@"\nexit 3\n', encoding="utf-8")
        program.chmod(0o755)
        env = {**os.environ, "BENCH_RUST_FIXTURE_TOOL": str(program),
               "BENCH_RUST_FIXTURE_TOOL_SHA256": rust_runtime.sha256_file(program)}
        done = subprocess.run([sys.executable, str(TOOL), "exec", "fixture-tool", "--", "roster", "--root", "a b", "--"],
                              env=env, capture_output=True, text=True, encoding="utf-8")
        self.assertEqual((done.returncode, done.stdout), (3, "roster|--root|a b|--|"), done.stderr)
        empty = self.root / "empty-store"
        done = subprocess.run([sys.executable, str(TOOL), "exec", "fixture-tool", "--", "roster"],
                              env={**os.environ, rust_runtime.STORE_VARIABLE: str(empty)},
                              capture_output=True, text=True, encoding="utf-8")
        self.assertEqual((done.returncode, done.stdout), (1, ""))
        self.assertIn(f"fixture-tool has no selected version in {empty}", done.stderr)
        self.assertIn("install it with:", done.stderr)

    def test_a_store_reached_through_a_symlink_resolves_to_its_canonical_path(self):
        real = self.root / "real"
        real.mkdir()
        (self.root / "linked").symlink_to(real, target_is_directory=True)
        self.store = self.root / "linked/store"

        def canonical(pin: dict) -> Path:
            return real / "store/fixture-tool" / pin["sha256"] / "fixture-tool"

        selected = self.head_version("selected")
        rust_runtime.select(self.store, "fixture-tool", selected)
        self.assertNotEqual(self.entry(selected) / "fixture-tool", canonical(selected))
        self.assertEqual(self.resolve(), canonical(selected))
        prints_its_path = script(self.root / "prints-its-path", '"$0"')
        frozen = rust_runtime.pin_of(rust_runtime.retain(self.store, prints_its_path, fake_receipt(prints_its_path))[0])
        run_dir = self.root / "run"
        run_dir.mkdir()
        (run_dir / "runtimes.frozen.json").write_text(json.dumps({"fixture-tool": frozen}), encoding="utf-8")
        self.assertEqual(self.resolve(run_dir), canonical(frozen))
        self.assertEqual(self.cli("resolve", "fixture-tool", "--run", str(run_dir)),
                         (0, json.dumps({"executable": str(canonical(frozen))}, indent=2) + "\n", ""))
        done = subprocess.run([sys.executable, str(TOOL), "exec", "fixture-tool", "--run", str(run_dir),
                               "--store", str(self.store)], capture_output=True, text=True, encoding="utf-8")
        self.assertEqual((done.returncode, done.stdout), (0, f"{canonical(frozen)}\n"), done.stderr)


class Interpreter(unittest.TestCase):
    def test_queue_commands_load_without_tomllib_and_only_install_needs_it(self):
        # Python 3.10 has no tomllib; a None entry in sys.modules makes its import fail the same way.
        probe = (
            "import sys; sys.modules['tomllib'] = None\n"
            "import review_queue, rust_runtime\n"
            "from pathlib import Path\n"
            "try:\n"
            "    rust_runtime.describe(Path('.'), rust_runtime.REPO, Path('/nonexistent'), {})\n"
            "except rust_runtime.InputError as error:\n"
            "    print(error)\n"
        )
        crate = self.enterContext(tempfile.TemporaryDirectory(dir=rust_runtime.REPO / "rust", prefix=".interpreter-test-"))
        done = subprocess.run([sys.executable, "-c", probe.replace("Path('.')", repr(crate).join(["Path(", ")"]))],
                              cwd=TOOL.parent, capture_output=True, text=True, encoding="utf-8")
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertIn("install needs Python 3.11 or later", done.stdout)


class PrivateCargoHome(unittest.TestCase):
    def test_a_build_home_shares_the_host_caches_and_their_locks_but_no_configuration(self):
        root = Path(self.enterContext(tempfile.TemporaryDirectory(prefix="rust-runtime-home-test-")))
        shared = root / "host-home"
        (shared / "registry").mkdir(parents=True)
        (shared / "config.toml").write_text(DEBUG_ASSERTIONS, encoding="utf-8")
        home = rust_runtime.private_home(root, {"CARGO_HOME": str(shared)})
        self.assertEqual(sorted(path.name for path in home.iterdir()), [".package-cache", ".package-cache-mutate", "registry"])
        self.assertEqual((home / "registry").resolve(), (shared / "registry").resolve())
        for lock in (".package-cache", ".package-cache-mutate"):
            with self.subTest(lock=lock), open(home / lock, "a") as build, open(shared / lock, "a") as other:
                fcntl.flock(build, fcntl.LOCK_EX)
                with self.assertRaises(BlockingIOError):
                    fcntl.flock(other, fcntl.LOCK_EX | fcntl.LOCK_NB)


class Deviations(Store):
    def setUp(self):
        super().setUp()
        self.run_dir = self.root / "runs/2026-10-18-fixture"
        self.run_dir.mkdir(parents=True)
        (self.run_dir / "manifest.json").write_text("{}", encoding="utf-8")

    def test_a_runtime_deviation_records_both_identities_once(self):
        one, two = self.version("one"), self.version("two")
        notes = ["--reason", "A parser fix.", "--scope", "New attempts of cells t/a/2 onward.",
                 "--validation", "Corpus and differential checks.", "--accounting", "No attempt is replaced; earlier attempts keep v1."]
        argv = ["deviation", "--run", str(self.run_dir), "--name", "fixture-tool", "--from", one["sha256"], "--to", two["sha256"], *notes]
        code, out, err = self.cli(*argv)
        self.assertEqual(code, 0, err)
        record = json.loads(Path(json.loads(out)["recorded"]).read_text(encoding="utf-8"))
        self.assertEqual((record["schema"], record["version"], record["old"]["pin"], record["new"]["pin"]),
                         (rust_runtime.DEVIATION, 1, one, two))
        self.assertEqual(record["new"]["receipt"]["artifact"]["sha256"], two["sha256"])
        self.assertNotIn("predecessor", record)
        self.assertNotIn("cause", record)
        code, _, err = self.cli(*argv)
        self.assertEqual(code, 1)
        self.assertIn("never overwritten", err)
        blank = [*argv[:-1], " "]
        code, _, err = self.cli(*blank)
        self.assertEqual(code, 2)
        self.assertIn("states accounting", err)


@unittest.skipUnless(shutil.which("cargo") or os.environ.get("RUST_RUNTIME_REQUIRE_CARGO") == "1", "cargo is unavailable")
class RealBuilds(unittest.TestCase):
    """Two committed versions of a small crate built by the active cargo, in a disposable repository."""

    def setUp(self):
        self.root = Path(self.enterContext(tempfile.TemporaryDirectory(prefix="rust-runtime-build-test-")))
        self.repo = self.root / "repo"
        self.crate = self.repo / "rust/fixture-tool"
        (self.crate / "src").mkdir(parents=True)
        self.git("init", "-q")
        self.release = dict(line.split(": ", 1) for line in subprocess.check_output(["rustc", "-vV"], cwd=self.crate, text=True)
                            .splitlines() if ": " in line)["release"]
        (self.crate / "Cargo.toml").write_text('[package]\nname = "fixture-tool"\nversion = "0.1.0"\nedition = "2021"\n'
                                               '[profile.release]\noverflow-checks = true\n', encoding="utf-8")
        (self.crate / "rust-toolchain.toml").write_text(f'[toolchain]\nchannel = "{self.release}"\n', encoding="utf-8")
        (self.crate / ".gitignore").write_text("/target/\n", encoding="utf-8")
        self.commit("one")
        self.store = self.root / "store"

    def git(self, *args: str) -> str:
        return subprocess.check_output(["git", "-C", str(self.repo), "-c", "user.name=Fixture",
                                        "-c", "user.email=fixture@example.invalid", *args], text=True)

    def commit(self, output: str) -> None:
        (self.crate / "src/main.rs").write_text(f'fn main() {{ println!("{output}"); }}\n', encoding="utf-8")
        if not (self.crate / "Cargo.lock").exists():
            subprocess.run(["cargo", "generate-lockfile", "--offline"], cwd=self.crate, check=True, capture_output=True)
        self.git("add", ".")
        self.git("commit", "-qm", output)

    def install(self) -> dict:
        with tempfile.TemporaryDirectory(dir=self.root) as scratch:
            executable, receipt = rust_runtime.build(self.crate, self.repo, Path(scratch))
            directory, _ = rust_runtime.retain(self.store, executable, receipt)
        return rust_runtime.pin_of(directory)

    def test_install_two_versions_roll_back_and_keep_each_pinned_operation_on_its_own(self):
        one = self.install()
        rust_runtime.select(self.store, "fixture-tool", one)
        self.commit("two")
        two = self.install()
        rust_runtime.select(self.store, "fixture-tool", two)
        self.assertNotEqual(one["sha256"], two["sha256"])
        started = rust_runtime.current_pin(self.store, "fixture-tool")
        rust_runtime.rollback(self.store, "fixture-tool")
        fresh = rust_runtime.current_pin(self.store, "fixture-tool")
        outputs = [subprocess.check_output([rust_runtime.check(self.store, "fixture-tool", pin)[0]], text=True)
                   for pin in (started, fresh)]
        self.assertEqual(outputs, ["two\n", "one\n"])
        _, receipt = rust_runtime.check(self.store, "fixture-tool", fresh)
        self.assertEqual(receipt["source"]["tree"], self.git("rev-parse", "HEAD~1:rust/fixture-tool").strip())
        self.assertIsNone(rust_runtime.source_problem(receipt, self.git("rev-parse", "HEAD~1").strip(), self.repo))
        self.assertIn("was built from crate tree", rust_runtime.source_problem(receipt, "HEAD", self.repo))
        self.assertNotIn(str(self.root), json.dumps(receipt))

    def test_a_rebuild_of_the_same_commit_from_another_checkout_has_the_same_hash(self):
        one = self.install()
        clone = self.root / "elsewhere/repo"
        subprocess.run(["git", "clone", "-q", str(self.repo), str(clone)], check=True)
        with tempfile.TemporaryDirectory(dir=self.root) as scratch:
            executable, receipt = rust_runtime.build(clone / "rust/fixture-tool", clone, Path(scratch))
            self.assertEqual(receipt["artifact"]["sha256"], one["sha256"])
            _, added = rust_runtime.retain(self.store, executable, receipt)
        self.assertFalse(added)

    def test_a_checkout_edit_saved_during_the_build_and_reverted_before_it_ends_is_not_compiled(self):
        real, main = rust_runtime.run, self.crate / "src/main.rs"
        committed = main.read_text(encoding="utf-8")
        for step in ("metadata", "build"):
            def edit_from(command, cwd, env=None):
                if command[:2] == ["cargo", step]:
                    main.write_text('fn main() { println!("edited"); }\n', encoding="utf-8")
                try:
                    return real(command, cwd, env)
                finally:
                    if command[:2] == ["cargo", "build"]:
                        main.write_text(committed, encoding="utf-8")

            with self.subTest(step=step), tempfile.TemporaryDirectory(dir=self.root) as scratch:
                with patch.object(rust_runtime, "run", edit_from):
                    executable, receipt = rust_runtime.build(self.crate, self.repo, Path(scratch))
                self.assertEqual(self.git("status", "--porcelain"), "")
                self.assertIsNone(rust_runtime.source_problem(receipt, self.git("rev-parse", "HEAD").strip(), self.repo))
                self.assertEqual(subprocess.check_output([executable], text=True), "one\n")

    def test_an_unclean_crate_another_toolchain_or_a_tracked_cargo_configuration_is_refused(self):
        with tempfile.TemporaryDirectory(dir=self.root) as scratch:
            (self.crate / "src/extra.rs").write_text("\n", encoding="utf-8")
            with self.assertRaisesRegex(rust_runtime.Refused, "uncommitted or untracked files"):
                rust_runtime.build(self.crate, self.repo, Path(scratch))
            (self.crate / "src/extra.rs").unlink()
            (self.crate / "rust-toolchain.toml").write_text('[toolchain]\nchannel = "1.0.0"\n', encoding="utf-8")
            self.git("commit", "-qam", "old toolchain")
            # Without an override rustup would switch to the pinned release before the check could see the mismatch.
            with patch.dict(os.environ, {"RUSTUP_TOOLCHAIN": self.release}):
                with self.assertRaisesRegex(rust_runtime.Refused, f"the active rustc is {self.release}; .* pins 1.0.0"):
                    rust_runtime.build(self.crate, self.repo, Path(scratch))
            (self.crate / "rust-toolchain.toml").write_text(f'[toolchain]\nchannel = "{self.release}"\n', encoding="utf-8")
            (self.crate / ".cargo").mkdir()
            (self.crate / ".cargo/config.toml").write_text(DEBUG_ASSERTIONS, encoding="utf-8")
            self.git("add", ".")
            self.git("commit", "-qm", "a tracked Cargo configuration")
            with self.assertRaisesRegex(rust_runtime.Refused, "cargo configuration a receipt cannot restore .*/.cargo/config.toml"):
                rust_runtime.build(self.crate, self.repo, Path(scratch))
        with tempfile.TemporaryDirectory(dir=self.root) as scratch, tempfile.TemporaryDirectory(dir=self.root) as cwd:
            # Stands in for /.cargo, which only an administrator can write.
            (Path(cwd) / ".cargo").mkdir()
            (Path(cwd) / ".cargo/config").write_text(DEBUG_ASSERTIONS, encoding="utf-8")
            with patch.object(rust_runtime, "CARGO_CWD", Path(cwd)):
                with self.assertRaisesRegex(rust_runtime.Refused, f"cargo configuration a receipt cannot restore .*{cwd}/.cargo/config"):
                    rust_runtime.build(self.crate, self.repo, Path(scratch))
        self.assertFalse(self.store.exists())

    def test_cargo_configuration_and_parent_workspaces_saved_outside_the_private_cargo_home_do_not_change_the_build(self):
        (self.crate / "src/main.rs").write_text('fn main() { println!("{}", cfg!(debug_assertions)); }\n', encoding="utf-8")
        self.git("commit", "-qam", "report debug assertions")
        host_home = self.root / "cargo-home"
        settings = [host_home / "config.toml", self.repo / ".cargo/config.toml", self.root / ".cargo/config.toml"]

        def build(scratch: Path, saved: list) -> str:
            """Builds while each ``(path, text)`` in ``saved`` exists only from the start of cargo build to its end."""
            real = rust_runtime.run

            def save_and_revert(command, cwd, env=None):
                if command[:2] != ["cargo", "build"]:
                    return real(command, cwd, env)
                manifest = Path(command[command.index("--manifest-path") + 1]) if "--manifest-path" in command else Path(cwd) / "Cargo.toml"
                files = [(path, text(manifest) if callable(text) else text) for path, text in saved]
                try:
                    for path, content in files:
                        path.parent.mkdir(parents=True, exist_ok=True)
                        path.write_text(content, encoding="utf-8")
                    return real(command, cwd, env)
                finally:
                    for path, _ in files:
                        path.unlink()

            with patch.dict(os.environ, {"CARGO_HOME": str(host_home)}), patch.object(rust_runtime, "run", save_and_revert):
                executable, receipt = rust_runtime.build(self.crate, self.repo, scratch)
            self.assertEqual(receipt["build"]["cargo_config_files"], [])
            self.assertEqual(self.git("status", "--porcelain", "--", "rust/fixture-tool"), "")
            return subprocess.check_output([executable], text=True)

        with self.subTest(when="saved before the install"), tempfile.TemporaryDirectory(dir=self.root) as scratch:
            for path in settings:
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(DEBUG_ASSERTIONS, encoding="utf-8")
            try:
                self.assertEqual(build(Path(scratch), []), "false\n")
            finally:
                for path in settings:
                    path.unlink()
        for path in settings:
            with self.subTest(when="saved during the build", path=path), tempfile.TemporaryDirectory(dir=self.root) as scratch:
                self.assertEqual(build(Path(scratch), [(path, DEBUG_ASSERTIONS)]), "false\n")
        with self.subTest(when="a workspace saved during the build"), tempfile.TemporaryDirectory(dir=self.root) as scratch:
            root = Path(scratch).resolve()
            claim = [(root / "Cargo.toml", lambda manifest: f'[workspace]\nmembers = ["{manifest.parent.relative_to(root).as_posix()}"]\n'
                      + DEBUG_ASSERTIONS),
                     (root / "Cargo.lock", (self.crate / "Cargo.lock").read_text(encoding="utf-8"))]
            self.assertEqual(build(Path(scratch), claim), "false\n")
        self.assertFalse(self.store.exists())

    def test_a_relative_build_directory_is_refused(self):
        # Cargo resolves these against the crate, where an ignored configuration file the check never sees can wait.
        (self.crate / ".gitignore").write_text("/target/\n/relative-home/\n", encoding="utf-8")
        self.git("commit", "-qam", "ignore a local Cargo home")
        (self.crate / "relative-home").mkdir()
        (self.crate / "relative-home/config.toml").write_text("[profile.release]\ndebug-assertions = true\n", encoding="utf-8")
        self.assertEqual(self.git("status", "--porcelain"), "")
        for name in ("CARGO_HOME", "HOME", "RUSTUP_HOME", "TMPDIR"):
            with self.subTest(name=name), tempfile.TemporaryDirectory(dir=self.root) as scratch:
                with patch.dict(os.environ, {name: "relative-home"}):
                    with self.assertRaisesRegex(rust_runtime.Refused, f"{name} must be absolute"):
                        rust_runtime.build(self.crate, self.repo, Path(scratch))
        self.assertFalse(self.store.exists())

    def test_build_metadata_git_does_not_track_or_a_source_override_is_refused(self):
        for name in ("Cargo.lock", "rust-toolchain.toml"):
            with self.subTest(name=name):
                self.git("rm", "-q", "--cached", f"rust/fixture-tool/{name}")
                (self.crate / ".gitignore").write_text(f"/target/\n/{name}\n", encoding="utf-8")
                self.git("commit", "-qam", f"ignore {name}")
                self.assertEqual(self.git("status", "--porcelain"), "")
                with tempfile.TemporaryDirectory(dir=self.root) as scratch:
                    with self.assertRaisesRegex(rust_runtime.Refused, f"rust/fixture-tool does not track {name}"):
                        rust_runtime.build(self.crate, self.repo, Path(scratch))
                (self.crate / ".gitignore").write_text("/target/\n", encoding="utf-8")
                self.git("add", ".")
                self.git("commit", "-qm", f"track {name} again")
        manifest = (self.crate / "Cargo.toml").read_text(encoding="utf-8")
        (self.crate / "Cargo.toml").write_text(manifest + '[patch.crates-io]\nserde = { path = "vendor/serde" }\n', encoding="utf-8")
        self.git("commit", "-qam", "a source override")
        with tempfile.TemporaryDirectory(dir=self.root) as scratch:
            with self.assertRaisesRegex(rust_runtime.Refused, r"Cargo.toml has \[patch\] sections"):
                rust_runtime.build(self.crate, self.repo, Path(scratch))
        self.assertFalse(self.store.exists())

    def test_a_crate_inside_another_cargo_workspace_is_refused(self):
        (self.repo / "rust/Cargo.toml").write_text('[workspace]\nmembers = ["fixture-tool"]\n[profile.release]\nopt-level = 0\n',
                                                   encoding="utf-8")
        subprocess.run(["cargo", "generate-lockfile", "--offline"], cwd=self.repo / "rust", check=True, capture_output=True)
        self.git("add", ".")
        self.git("commit", "-qm", "a parent workspace")
        with tempfile.TemporaryDirectory(dir=self.root) as scratch:
            with self.assertRaisesRegex(rust_runtime.Refused, "rust/fixture-tool belongs to the Cargo workspace .*rust/Cargo.toml"):
                rust_runtime.build(self.crate, self.repo, Path(scratch))
        self.assertFalse(self.store.exists())

    def test_a_build_sees_only_the_passed_through_environment_and_may_not_read_it(self):
        (self.crate / "src/main.rs").write_text('fn main() { println!("{}", env!("RUNTIME_FLAVOR")); }\n', encoding="utf-8")
        self.git("commit", "-qam", "read an environment variable")
        with tempfile.TemporaryDirectory(dir=self.root) as scratch, patch.dict(os.environ, {"RUNTIME_FLAVOR": "one"}):
            with self.assertRaisesRegex(rust_runtime.InputError, "RUNTIME_FLAVOR"):
                rust_runtime.build(self.crate, self.repo, Path(scratch))
        (self.crate / "src/main.rs").write_text('fn main() { println!("{:?}", option_env!("HOME")); }\n', encoding="utf-8")
        self.git("commit", "-qam", "read HOME")
        with tempfile.TemporaryDirectory(dir=self.root) as scratch:
            with self.assertRaisesRegex(rust_runtime.Refused, "read host environment variables its receipt cannot pin: HOME"):
                rust_runtime.build(self.crate, self.repo, Path(scratch))
        (self.crate / "src/main.rs").write_text('fn main() { println!("one"); }\n', encoding="utf-8")
        (self.crate / "build.rs").write_text('fn main() { println!("cargo:rerun-if-env-changed=PATH"); }\n', encoding="utf-8")
        self.git("add", ".")
        self.git("commit", "-qm", "a build script that reads PATH")
        with tempfile.TemporaryDirectory(dir=self.root) as scratch:
            with self.assertRaisesRegex(rust_runtime.Refused, "receipt cannot pin: PATH"):
                rust_runtime.build(self.crate, self.repo, Path(scratch))
        (self.crate / "build.rs").unlink()
        self.git("commit", "-qam", "no build script")
        with tempfile.TemporaryDirectory(dir=self.root) as scratch, patch.dict(os.environ, {"RUSTFLAGS": "-C opt-level=0"}):
            _, receipt = rust_runtime.build(self.crate, self.repo, Path(scratch))
        self.assertEqual(receipt["build"]["environment"],
                         sorted({name for name in rust_runtime.BUILD_ENVIRONMENT if name in os.environ} | {"CARGO_HOME", "PATH"}))
        self.assertFalse(self.store.exists())

    def test_a_registry_dependency_missing_from_the_cargo_cache_names_the_fetch_step(self):
        manifest = (self.crate / "Cargo.toml").read_text(encoding="utf-8")
        (self.crate / "Cargo.toml").write_text(manifest + '[dependencies]\ncfg-if = "=1.0.5"\n', encoding="utf-8")
        (self.crate / "Cargo.lock").write_text(
            "version = 4\n\n[[package]]\nname = \"cfg-if\"\nversion = \"1.0.5\"\n"
            "source = \"registry+https://github.com/rust-lang/crates.io-index\"\n"
            "checksum = \"4e7648175b45a9a48536d676f68d918270699102aa8dab5496df06904c914600\"\n\n"
            "[[package]]\nname = \"fixture-tool\"\nversion = \"0.1.0\"\ndependencies = [\n \"cfg-if\",\n]\n",
            encoding="utf-8")
        self.commit("registry dependency")
        empty = self.root / "empty-cargo-home"
        empty.mkdir()
        rustup = os.environ.get("RUSTUP_HOME") or str(Path.home() / ".rustup")
        with patch.dict(os.environ, {"CARGO_HOME": str(empty), "RUSTUP_HOME": rustup}), \
                tempfile.TemporaryDirectory(dir=self.root) as scratch:
            with self.assertRaisesRegex(rust_runtime.InputError, "(?s)cfg-if.*run `cargo fetch --locked` in rust/fixture-tool"):
                rust_runtime.build(self.crate, self.repo, Path(scratch))
        self.assertFalse(self.store.exists())

    def test_a_build_that_reads_local_files_git_does_not_track_in_the_crate_is_refused(self):
        dependency = self.repo / "rust/sibling"
        (dependency / "src").mkdir(parents=True)
        (dependency / "Cargo.toml").write_text('[package]\nname = "sibling"\nversion = "0.1.0"\nedition = "2021"\n', encoding="utf-8")
        (dependency / "src/lib.rs").write_text("pub fn value() -> u8 { 1 }\n", encoding="utf-8")
        manifest = (self.crate / "Cargo.toml").read_text(encoding="utf-8")
        # The build compiles a snapshot of the crate tree, so only an absolute path still reaches a file outside it.
        for path in (str(dependency), "../sibling"):
            for optional in (False, True):
                (self.crate / "Cargo.toml").write_text(manifest + f"[dependencies]\nsibling = {{ path = {json.dumps(path)}"
                                                       + (", optional = true }\n" if optional else " }\n"), encoding="utf-8")
                (self.crate / "src/main.rs").write_text('fn main() { println!("one"); }\n' if optional else
                                                        'fn main() { println!("{}", sibling::value()); }\n', encoding="utf-8")
                subprocess.run(["cargo", "generate-lockfile", "--offline"], cwd=self.crate, check=True, capture_output=True)
                self.git("add", ".")
                self.git("commit", "-qm", f"path dependency {path}, optional {optional}")
                with self.subTest(path=path, optional=optional), tempfile.TemporaryDirectory(dir=self.root) as scratch:
                    if path == "../sibling":
                        with self.assertRaisesRegex(rust_runtime.InputError, "(?s)cargo metadata .*sibling"):
                            rust_runtime.build(self.crate, self.repo, Path(scratch))
                    else:
                        with self.assertRaisesRegex(rust_runtime.Refused,
                                                    "depends on local packages whose manifests it does not track: .*sibling/Cargo.toml"):
                            rust_runtime.build(self.crate, self.repo, Path(scratch))
        (self.crate / "Cargo.toml").write_text(manifest, encoding="utf-8")
        (self.repo / "outside.txt").write_text("outside\n", encoding="utf-8")
        for path in (str(self.repo / "outside.txt"), "../../../outside.txt"):
            (self.crate / "src/main.rs").write_text(f'fn main() {{ print!("{{}}", include_str!({json.dumps(path)})); }}\n', encoding="utf-8")
            subprocess.run(["cargo", "generate-lockfile", "--offline"], cwd=self.crate, check=True, capture_output=True)
            self.git("add", ".")
            self.git("commit", "-qm", f"include {path}")
            with self.subTest(path=path), tempfile.TemporaryDirectory(dir=self.root) as scratch:
                if path.startswith(".."):
                    with self.assertRaisesRegex(rust_runtime.InputError, "(?s)cargo build .*outside.txt"):
                        rust_runtime.build(self.crate, self.repo, Path(scratch))
                else:
                    with self.assertRaisesRegex(rust_runtime.Refused, "not tracked in rust/fixture-tool.*outside.txt"):
                        rust_runtime.build(self.crate, self.repo, Path(scratch))
        (self.crate / ".gitignore").write_text("/target/\n/ignored.txt\n", encoding="utf-8")
        (self.crate / "ignored.txt").write_text("ignored\n", encoding="utf-8")
        (self.crate / "src/main.rs").write_text('fn main() { print!("{}", include_str!("../ignored.txt")); }\n', encoding="utf-8")
        self.git("add", ".")
        self.git("commit", "-qm", "include an ignored file")
        self.assertEqual(self.git("status", "--porcelain"), "")
        with tempfile.TemporaryDirectory(dir=self.root) as scratch:
            with self.assertRaisesRegex(rust_runtime.InputError, "(?s)cargo build .*ignored.txt"):
                rust_runtime.build(self.crate, self.repo, Path(scratch))
        (self.crate / "ignored.txt").unlink()
        (self.crate / "src/value.txt").symlink_to("../../../outside.txt")
        (self.crate / "src/main.rs").write_text('fn main() { print!("{}", include_str!("value.txt")); }\n', encoding="utf-8")
        self.git("add", ".")
        self.git("commit", "-qm", "include a tracked symbolic link")
        self.assertEqual(self.git("status", "--porcelain"), "")
        with tempfile.TemporaryDirectory(dir=self.root) as scratch:
            with self.assertRaisesRegex(rust_runtime.Refused, "rust/fixture-tool tracks symbolic links.*rust/fixture-tool/src/value.txt"):
                rust_runtime.build(self.crate, self.repo, Path(scratch))
        self.assertFalse(self.store.exists())


if __name__ == "__main__":
    unittest.main()
