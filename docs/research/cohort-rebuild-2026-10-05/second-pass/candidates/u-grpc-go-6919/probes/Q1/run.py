import os, pathlib, shutil, subprocess
repo = pathlib.Path.cwd()
out = repo / "docs/research/cohort-rebuild-2026-10-05/second-pass/candidates/u-grpc-go-6919/probes/Q1"
scratch = pathlib.Path("<scratch>")
clone = scratch / "clone"
env = dict(os.environ, GOPATH=str(scratch / "gopath"), GOMODCACHE=str(scratch / "gomodcache"), GOCACHE=str(scratch / "gocache"), GOTMPDIR=str(scratch / "tmp"), GOTOOLCHAIN="local", GOENV="off")
(scratch / "tmp").mkdir(exist_ok=True)
versions = subprocess.check_output(["go", "version"], env=env, text=True)
versions += subprocess.check_output(["git", "--version"], text=True)
versions += subprocess.check_output(["gh", "--version"], text=True)
versions += "\n" + str({k:env[k] for k in ["GOPATH","GOMODCACHE","GOCACHE","GOTMPDIR","GOTOOLCHAIN","GOENV"]}) + "\n"
for label, sha in [("base", "5051eeae537cb2839dd499e1a63a141098a3a03a"), ("head", "b8374114d485b6957b15d8769d7d5d96ddeaafc6")]:
 subprocess.run(["git", "-C", str(clone), "checkout", "--detach", sha], check=True)
 target = clone / "dossierprobe"
 target.mkdir(exist_ok=True)
 shutil.copyfile(out / "main.go", target / "main.go")
 result = subprocess.run(["go", "run", "-mod=readonly", "./dossierprobe"], cwd=clone, env=env, capture_output=True, text=True)
 (out / ("result-" + label + ".txt")).write_text("commit=" + sha + "\ncommand=go run -mod=readonly ./dossierprobe\nexit=" + str(result.returncode) + "\n" + result.stdout + result.stderr)
 print(label, result.returncode, result.stdout, result.stderr, flush=True)
 versions += label + "\n" + subprocess.check_output(["go", "list", "-m", "github.com/golang/protobuf", "google.golang.org/protobuf"], cwd=clone, env=env, text=True)
(out / "environment.txt").write_text(versions)
