"""Select grading coverage from the complete PR diff, defaulting to full coverage."""
import argparse
import json
from pathlib import Path
import subprocess


ROOT = Path(__file__).resolve().parents[1]
UI_PREFIXES = ("src/components/", "src/routes/")
ASSET_SUFFIXES = {".css", ".svg", ".png", ".jpg", ".jpeg", ".webp", ".ico", ".woff", ".woff2"}
UNRELATED_FILES = {"src/styles.css", ".github/workflows/rust-runtime.yml"}


def unrelated(path: str) -> bool:
    return (path.startswith(UI_PREFIXES) and Path(path).suffix in {".tsx", ".css"}
            or path in UNRELATED_FILES
            or path.startswith("public/") and Path(path).suffix in ASSET_SUFFIXES)


def selection(event_name: str, event: dict, root: Path = ROOT) -> tuple[bool, str]:
    if event_name != "pull_request":
        return True, "Main and manual runs retain full coverage"
    pr = event["pull_request"]
    base, head = pr["base"]["sha"], pr["head"]["sha"]
    paths = subprocess.check_output(
        ["git", "diff", "--name-only", "--no-renames", "-z", f"{base}...{head}", "--"],
        cwd=root,
    ).decode("utf-8").split("\0")
    paths = [path for path in paths if path]
    if not paths:
        return True, "Empty change detection retains full coverage"
    relevant = [path for path in paths if not unrelated(path)]
    if relevant:
        return True, f"Full coverage for {len(relevant)} relevant or unknown paths"
    return False, "Complete PR diff contains only UI, asset or Rust runtime workflow paths"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--event-name", required=True)
    parser.add_argument("--event-path", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    try:
        run, reason = selection(args.event_name, json.loads(args.event_path.read_text()))
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
        run, reason = True, f"Change detection unavailable: {error}"
    print(reason)
    with args.output.open("a") as output:
        output.write(f"run={'true' if run else 'false'}\n")


if __name__ == "__main__":
    main()
