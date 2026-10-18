"""Prove that pinned Rust executables are retained, checked, selected and rolled back without touching pinned operations.

Most tests use shell scripts as executables in disposable stores. ``RealBuilds`` builds a small crate with the active
cargo in a disposable repository; it is skipped without cargo unless ``RUST_RUNTIME_REQUIRE_CARGO=1``.
"""
from __future__ import annotations

from contextlib import redirect_stderr, redirect_stdout
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import rust_runtime

TOOL = Path(__file__).resolve().with_name("rust_runtime.py")


def fake_receipt(executable: Path, name: str = "fixture-tool", **changes) -> dict:
    receipt = {
        "schema": rust_runtime.RECEIPT,
        "crate": {"name": name, "version": "0.1.0", "path": f"rust/{name}", "bin": name},
        "source": {"repository": None, "revision": "a" * 40, "tree": "b" * 40, "cargo_toml_sha256": "c" * 64,
                   "cargo_lock_sha256": "d" * 64, "toolchain_file_sha256": "e" * 64},
        "toolchain": {"channel": "1.99.1", "rustc": {"release": "1.99.1", "host": rust_runtime.host_target()},
                      "cargo": "cargo 1.99.1"},
        "target": rust_runtime.host_target(), "libc": rust_runtime.host_libc(),
        "build": {"command": ["cargo", "build"], "rustflags": [], "cleared_environment": "", "cargo_config_files": []},
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
        self.root = Path(self.enterContext(tempfile.TemporaryDirectory(prefix="rust-runtime-test-")))
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

    def test_a_binary_for_another_target_libc_or_an_unverified_host_is_refused(self):
        other = self.version("mac", target="aarch64-apple-darwin")
        with self.assertRaisesRegex(rust_runtime.Refused, "built for aarch64-apple-darwin; this host is"):
            rust_runtime.check(self.store, "fixture-tool", other)
        newer = self.version("newer-libc", libc={"name": "glibc", "version": "99.0"})
        with self.assertRaisesRegex(rust_runtime.Refused, "links glibc 99.0; this host has"):
            rust_runtime.check(self.store, "fixture-tool", newer)
        musl = self.version("musl", libc={"name": "musl", "version": "1.2"})
        with self.assertRaisesRegex(rust_runtime.Refused, "links musl 1.2"):
            rust_runtime.check(self.store, "fixture-tool", musl)
        arm = self.version("arm", target="aarch64-unknown-linux-gnu")
        with patch.object(rust_runtime, "host_target", return_value="aarch64-unknown-linux-gnu"):
            with self.assertRaisesRegex(rust_runtime.Refused, "aarch64-unknown-linux-gnu is not a verified Rust runtime target"):
                rust_runtime.check(self.store, "fixture-tool", arm)

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


class Interpreter(unittest.TestCase):
    def test_queue_commands_load_without_tomllib_and_only_install_needs_it(self):
        # Python 3.10 has no tomllib; a None entry in sys.modules makes its import fail the same way.
        probe = (
            "import sys; sys.modules['tomllib'] = None\n"
            "import review_queue, rust_runtime\n"
            "from pathlib import Path\n"
            "try:\n"
            "    rust_runtime.describe(Path('.'), rust_runtime.REPO, {})\n"
            "except rust_runtime.InputError as error:\n"
            "    print(error)\n"
        )
        crate = self.enterContext(tempfile.TemporaryDirectory(dir=rust_runtime.REPO / "rust", prefix=".interpreter-test-"))
        done = subprocess.run([sys.executable, "-c", probe.replace("Path('.')", repr(crate).join(["Path(", ")"]))],
                              cwd=TOOL.parent, capture_output=True, text=True, encoding="utf-8")
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertIn("install needs Python 3.11 or later", done.stdout)


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
        release = dict(line.split(": ", 1) for line in subprocess.check_output(["rustc", "-vV"], cwd=self.crate, text=True)
                       .splitlines() if ": " in line)["release"]
        (self.crate / "Cargo.toml").write_text('[package]\nname = "fixture-tool"\nversion = "0.1.0"\nedition = "2021"\n'
                                               '[profile.release]\noverflow-checks = true\n', encoding="utf-8")
        (self.crate / "rust-toolchain.toml").write_text(f'[toolchain]\nchannel = "{release}"\n', encoding="utf-8")
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

    def test_an_unclean_crate_another_toolchain_or_unrestorable_cargo_configuration_is_refused(self):
        with tempfile.TemporaryDirectory(dir=self.root) as scratch:
            (self.crate / "src/extra.rs").write_text("\n", encoding="utf-8")
            with self.assertRaisesRegex(rust_runtime.Refused, "uncommitted or untracked files"):
                rust_runtime.build(self.crate, self.repo, Path(scratch))
            (self.crate / "src/extra.rs").unlink()
            (self.crate / "rust-toolchain.toml").write_text('[toolchain]\nchannel = "1.0.0"\n', encoding="utf-8")
            self.git("commit", "-qam", "old toolchain")
            with self.assertRaisesRegex(rust_runtime.Refused, "pins 1.0.0"):
                rust_runtime.build(self.crate, self.repo, Path(scratch))
            home = self.root / "cargo-home"
            home.mkdir()
            (home / "config.toml").write_text("[build]\nrustflags = []\n", encoding="utf-8")
            with patch.dict(os.environ, {"CARGO_HOME": str(home)}):
                with self.assertRaisesRegex(rust_runtime.Refused, "cargo configuration a receipt cannot restore"):
                    rust_runtime.build(self.crate, self.repo, Path(scratch))
        self.assertFalse(self.store.exists())

    def test_a_build_that_reads_local_files_git_does_not_track_in_the_crate_is_refused(self):
        dependency = self.repo / "rust/sibling"
        (dependency / "src").mkdir(parents=True)
        (dependency / "Cargo.toml").write_text('[package]\nname = "sibling"\nversion = "0.1.0"\nedition = "2021"\n', encoding="utf-8")
        (dependency / "src/lib.rs").write_text("pub fn value() -> u8 { 1 }\n", encoding="utf-8")
        manifest = (self.crate / "Cargo.toml").read_text(encoding="utf-8")
        (self.crate / "Cargo.toml").write_text(manifest + '[dependencies]\nsibling = { path = "../sibling" }\n', encoding="utf-8")
        (self.crate / "src/main.rs").write_text('fn main() { println!("{}", sibling::value()); }\n', encoding="utf-8")
        subprocess.run(["cargo", "generate-lockfile", "--offline"], cwd=self.crate, check=True, capture_output=True)
        self.git("add", ".")
        self.git("commit", "-qm", "path dependency")
        with tempfile.TemporaryDirectory(dir=self.root) as scratch:
            with self.assertRaisesRegex(rust_runtime.Refused, "not tracked in rust/fixture-tool.*sibling/src/lib.rs"):
                rust_runtime.build(self.crate, self.repo, Path(scratch))
        (self.crate / "Cargo.toml").write_text(manifest, encoding="utf-8")
        (self.repo / "outside.txt").write_text("outside\n", encoding="utf-8")
        (self.crate / "src/main.rs").write_text('fn main() { print!("{}", include_str!("../../../outside.txt")); }\n', encoding="utf-8")
        subprocess.run(["cargo", "generate-lockfile", "--offline"], cwd=self.crate, check=True, capture_output=True)
        self.git("add", ".")
        self.git("commit", "-qm", "include an outside file")
        with tempfile.TemporaryDirectory(dir=self.root) as scratch:
            with self.assertRaisesRegex(rust_runtime.Refused, "not tracked in rust/fixture-tool.*outside.txt"):
                rust_runtime.build(self.crate, self.repo, Path(scratch))
        (self.crate / ".gitignore").write_text("/target/\n/ignored.txt\n", encoding="utf-8")
        (self.crate / "ignored.txt").write_text("ignored\n", encoding="utf-8")
        (self.crate / "src/main.rs").write_text('fn main() { print!("{}", include_str!("../ignored.txt")); }\n', encoding="utf-8")
        self.git("add", ".")
        self.git("commit", "-qm", "include an ignored file")
        self.assertEqual(self.git("status", "--porcelain"), "")
        with tempfile.TemporaryDirectory(dir=self.root) as scratch:
            with self.assertRaisesRegex(rust_runtime.Refused, "not tracked in rust/fixture-tool.*rust/fixture-tool/ignored.txt"):
                rust_runtime.build(self.crate, self.repo, Path(scratch))
        self.assertFalse(self.store.exists())


if __name__ == "__main__":
    unittest.main()
