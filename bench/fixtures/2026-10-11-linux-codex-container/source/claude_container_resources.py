#!/usr/bin/env python3
"""Check the Claude fixture's effective kernel limits using an existing pinned image.

This unpaid prerequisite starts no native client or provider. It does not establish
issue #36 conformance. Run it in the same delegated session as the intended fixture.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import uuid

SCHEMA = "claude-container-resource-probe/1"
LIMITS = {"memory.max": 1073741824, "pids.max": 256, "cpu.max": [200000, 100000]}
SAMPLE = """from pathlib import Path
import json
root = Path('/sys/fs/cgroup')
print(json.dumps({'cgroup': Path('/proc/self/cgroup').read_text(),
                 'limits': {name: (root / name).read_text() if (root / name).exists() else None
                            for name in ('memory.max', 'pids.max', 'cpu.max')}}))
"""


def effective_limits(sample):
    observed = sample.get("limits", {})
    failures = []
    for name in ("memory.max", "pids.max"):
        value = observed.get(name)
        if not isinstance(value, str) or not value.strip().isdigit() or int(value) != LIMITS[name]:
            failures.append(f"{name}: expected {LIMITS[name]}, observed {value!r}")
    value = observed.get("cpu.max")
    if not isinstance(value, str) or value.split() != list(map(str, LIMITS["cpu.max"])):
        failures.append(f"cpu.max: expected '200000 100000', observed {value!r}")
    return failures


def write(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


def command(argv, output, label, *, timeout=45):
    write(output / (label + "-command.json"), argv)
    with (output / (label + "-stdout.txt")).open("w", encoding="utf-8") as stdout, \
            (output / (label + "-stderr.txt")).open("w", encoding="utf-8") as stderr:
        result = subprocess.run(argv, stdout=stdout, stderr=stderr, timeout=timeout, check=False)
    return result.returncode


def probe(args):
    output = args.output.resolve()
    output.mkdir(mode=0o700)
    receipt = {"schema": SCHEMA, "passed": False, "issue36_complete": False,
               "provider_calls": 0, "native_client_started": False, "failures": [],
               "requested_limits": LIMITS}
    name = "claude-resource-" + uuid.uuid4().hex
    podman = None
    launched = False
    write(output / "receipt.json", receipt)
    try:
        if sys.platform != "linux" or os.getuid() == 0:
            raise ValueError("only unprivileged Linux is supported")
        if not re.fullmatch(r"sha256:[0-9a-f]{64}", args.image):
            raise ValueError("image must be an immutable sha256 image ID")
        runtime = args.oci_runtime.resolve(strict=True)
        runtime_hash = hashlib.sha256(runtime.read_bytes()).hexdigest()
        if runtime_hash != args.oci_runtime_sha256:
            raise ValueError("OCI runtime hash does not match its pin")
        podman = shutil.which(args.podman)
        if not podman:
            raise ValueError("Podman is not installed")
        receipt.update(container=name, image=args.image, kernel=os.uname().release,
                       cgroup_manager=args.cgroup_manager,
                       oci_runtime={"path": str(runtime), "sha256": runtime_hash},
                       controller_cgroup=Path("/proc/self/cgroup").read_text(),
                       script_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest())
        if command([podman, "info", "--format", "json"], output, "info"):
            raise ValueError("Podman info failed; see info-stderr.txt")
        info = json.loads((output / "info-stdout.txt").read_text())
        if not info.get("host", {}).get("security", {}).get("rootless"):
            raise ValueError("Podman is not rootless")
        if command([str(runtime), "--version"], output, "runtime"):
            raise ValueError("OCI runtime version command failed")
        if command([podman, "image", "inspect", args.image], output, "image"):
            raise ValueError("pinned fixture image is not available locally")
        image = json.loads((output / "image-stdout.txt").read_text())
        if len(image) != 1 or image[0].get("Id") not in (args.image, args.image.removeprefix("sha256:")):
            raise ValueError("resolved image differs from its pin")
        launched = True
        argv = [podman, "--runtime", str(runtime), "--cgroup-manager", args.cgroup_manager,
                "run", "--name", name, "--pull", "never", "--network", "none",
                "--pid", "private", "--ipc", "private", "--uts", "private",
                "--userns", "keep-id", "--user", f"{os.getuid()}:{os.getgid()}",
                "--cap-drop", "ALL", "--security-opt", "no-new-privileges", "--read-only",
                "--pids-limit", "256", "--memory", "1g", "--cpus", "2", "--timeout", "30",
                "--log-driver", "k8s-file", "--log-opt", "max-size=1m", "--shm-size", "64m",
                "--tmpfs", "/tmp:rw,nosuid,nodev,size=256m",
                "--tmpfs", "/run:rw,nosuid,nodev,size=16m",
                "--tmpfs", "/var/tmp:rw,nosuid,nodev,size=16m",
                "--entrypoint", "/usr/bin/python3", args.image, "-c", SAMPLE]
        receipt["container_exit"] = command(argv, output, "run")
        if receipt["container_exit"]:
            raise ValueError("capped container failed; see run-stderr.txt")
        sample = json.loads((output / "run-stdout.txt").read_text())
        receipt["effective"] = sample
        receipt["failures"].extend(effective_limits(sample))
        receipt["passed"] = not receipt["failures"]
    except (OSError, ValueError, subprocess.TimeoutExpired) as error:
        receipt["failures"].append(str(error))
    finally:
        if launched:
            try:
                inspected = command([podman, "inspect", name], output, "inspect")
                if inspected == 0:
                    state = json.loads((output / "inspect-stdout.txt").read_text())[0]["State"]
                    if state["Running"] and command([podman, "stop", "--time", "3", name], output, "stop"):
                        raise ValueError("failed to stop owned resource probe")
                    if command([podman, "rm", name], output, "cleanup"):
                        raise ValueError("failed to remove owned resource probe")
                    if command([podman, "container", "exists", name], output, "absent") != 1:
                        raise ValueError("owned resource probe removal could not be verified")
                    receipt["cleanup"] = "removed after retained inspection"
                else:
                    receipt["passed"] = False
                    receipt["cleanup"] = "inspection failed; see inspect-stderr.txt"
                    receipt["failures"].append("owned resource probe could not be inspected")
            except (OSError, ValueError, KeyError, subprocess.TimeoutExpired) as error:
                receipt["passed"] = False
                receipt["failures"].append(str(error))
        write(output / "receipt.json", receipt)
    print(json.dumps({"passed": receipt["passed"], "receipt": str(output / "receipt.json")}))
    return 0 if receipt["passed"] else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--image", required=True)
    parser.add_argument("--oci-runtime", type=Path, required=True)
    parser.add_argument("--oci-runtime-sha256", required=True)
    parser.add_argument("--podman", default="podman")
    parser.add_argument("--cgroup-manager", choices=("cgroupfs", "systemd"), default="cgroupfs")
    return probe(parser.parse_args())


if __name__ == "__main__":
    try:
        sys.exit(main())
    except OSError as error:
        print(f"claude_container_resources.py: {error}", file=sys.stderr)
        sys.exit(2)
