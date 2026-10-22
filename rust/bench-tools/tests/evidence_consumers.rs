//! Cold retrieval preserves frozen records through the existing evidence consumers.

use bench_tools::pyjson::{self, Dumps, Duplicates, Value};
use bench_tools::sha256::hex_digest;
use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const REPOSITORY: &str = "owner/canonical";
const TAG: &str = "evidence-fixture";

/// The release routes of the corpora's gh stub, answering from the release named in `release` and the assets
/// stored beside it, and appending each argument list to calls.jsonl. The test's tag needs no percent-encoding.
const GH_STUB: &str = r#"#!/bin/sh
export LC_ALL=C
state=$CONTRACT_GH_STATE
mkdir -p "$state"
line= separator=
for argument do
  line="$line$separator\"$(printf '%s' "$argument" | sed 's/[\\"]/\\&/g')\""
  separator=', '
done
printf '[%s]\n' "$line" >> "$state/calls.jsonl"
fail() {
  printf '%s\n' "$1" >&2
  exit "${2:-1}"
}
[ -f "$state/release" ] || fail 'HTTP 404: Not Found'
. "$state/release"
routes="repos/$repository/releases/"
stored=$state/assets
assets() {
  [ -d "$stored" ] || return 0
  for asset in $(ls "$stored" | sort -n); do
    for file in $(ls "$stored/$asset" | sort); do
      printf '%s %s\n' "$asset" "$file"
    done
  done
}
listed=$(assets)
if [ $# -eq 2 ] && [ "$1" = api ] && [ "$2" = "${routes}tags/$tag" ]; then
  printf '{"id": %s, "tag_name": "%s", "draft": false}\n' "$id" "$tag"
elif [ $# -eq 4 ] && [ "$1 $2 $3" = 'api --paginate --slurp' ] && [ "$4" = "$routes$id/assets?per_page=100" ]; then
  pages='[[' count=0
  while read -r asset file; do
    [ -n "$asset" ] || continue
    if [ $count -gt 0 ]; then
      if [ $((count % 100)) -eq 0 ]; then pages="$pages], ["; else pages="$pages, "; fi
    fi
    size=$(wc -c < "$stored/$asset/$file" | tr -d ' ')
    pages="$pages{\"id\": $asset, \"name\": \"$file\", \"size\": $size}"
    count=$((count + 1))
  done <<EOF
$listed
EOF
  printf '%s]]\n' "$pages"
elif [ $# -eq 6 ] && [ "$1 $2" = 'release upload' ] && [ "$3" = "$tag" ] && [ "$5" = --repo ] && [ "$6" = "$repository" ]; then
  name=$(basename "$4")
  last=100
  while read -r asset file; do
    [ -n "$asset" ] || continue
    [ "$file" != "$name" ] || fail 'asset under the same name already exists'
    [ "$asset" -le "$last" ] || last=$asset
  done <<EOF
$listed
EOF
  target="$stored/$((last + 1))"
  mkdir -p "$target" && cp "$4" "$target/$name"
elif [ $# -eq 4 ] && [ "$1" = api ] && [ "$3" = -H ] && [ "$4" = 'Accept: application/octet-stream' ]; then
  while read -r asset file; do
    if [ -n "$asset" ] && [ "$2" = "${routes}assets/$asset" ]; then
      exec cat "$stored/$asset/$file"
    fi
  done <<EOF
$listed
EOF
  fail 'HTTP 404: Not Found'
else
  fail 'contract gh stub: unsupported arguments' 2
fi
"#;

/// A test's temporary directory, holding the gh stub on a directory of its own.
struct Scratch {
    base: PathBuf,
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

impl Scratch {
    fn new(label: &str) -> Self {
        let base = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("evidence-consumers-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let stub = base.join("gh-stub/gh");
        fs::create_dir_all(stub.parent().unwrap()).unwrap();
        fs::write(&stub, GH_STUB).unwrap();
        fs::set_permissions(&stub, fs::Permissions::from_mode(0o755)).unwrap();
        Self {
            base: fs::canonicalize(&base).unwrap(),
        }
    }

    /// `bench-tools` in `cwd` with the stub gh answering from the release, or from no release.
    fn run(
        &self,
        release: Option<&Release>,
        cwd: &Path,
        arguments: &[&dyn AsRef<OsStr>],
    ) -> Output {
        let stub = self.base.join("gh-stub");
        let state = release.map_or_else(|| stub.join("state"), |release| release.state.clone());
        let path = std::env::var_os("PATH").unwrap_or_default();
        let path =
            std::env::join_paths(std::iter::once(stub.clone()).chain(std::env::split_paths(&path)))
                .unwrap();
        bench_tools(cwd, arguments)
            .env("PATH", path)
            .env("CONTRACT_GH_STATE", state)
            .output()
            .unwrap()
    }

    fn collect(&self, release: Option<&Release>, root: &Path, run: &Path) -> Output {
        self.run(release, root, &[&"evidence", &"collect", &"--run", &run])
    }

    fn native(&self, arguments: &[&dyn AsRef<OsStr>]) -> Output {
        bench_tools(&self.base, &[&"native-artifacts" as &dyn AsRef<OsStr>])
            .args(arguments)
            .output()
            .unwrap()
    }

    /// Packs the selection and publishes it to the release, writing its manifest.
    fn publish(&self, release: &Release, selection: &str, manifest: &Path) {
        let selected = self.base.join("selection.json");
        write(&selected, selection);
        let packed = self.base.join("packed");
        succeeded(&self.run(
            Some(release),
            &self.base,
            &[
                &"evidence",
                &"pack",
                &"--selection",
                &selected,
                &"--out",
                &packed,
            ],
        ));
        succeeded(&self.run(
            Some(release),
            &self.base,
            &[
                &"evidence",
                &"publish",
                &"--manifest",
                &packed.join("manifest.json"),
                &"--out",
                &manifest,
            ],
        ));
    }
}

/// The corpora's stub gh release, kept in one state directory across bench-tools calls.
struct Release {
    state: PathBuf,
}

impl Release {
    fn new(base: &Path) -> Self {
        let state = base.join("gh-state");
        fs::create_dir_all(&state).unwrap();
        fs::write(
            state.join("release"),
            format!("repository={REPOSITORY}\ntag={TAG}\nid=42\n"),
        )
        .unwrap();
        Self { state }
    }

    fn assets(&self) -> Vec<PathBuf> {
        let mut found: Vec<(u64, PathBuf)> = Vec::new();
        for directory in fs::read_dir(self.state.join("assets")).unwrap() {
            let directory = directory.unwrap();
            let id = directory.file_name().to_str().unwrap().parse().unwrap();
            for file in fs::read_dir(directory.path()).unwrap() {
                found.push((id, file.unwrap().path()));
            }
        }
        found.sort();
        found.into_iter().map(|(_, path)| path).collect()
    }

    fn calls(&self) -> Vec<Vec<String>> {
        fs::read_to_string(self.state.join("calls.jsonl"))
            .unwrap()
            .lines()
            .map(|line| {
                pyjson::loads(line, Duplicates::KeepLast)
                    .unwrap()
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|argument| argument.as_str().unwrap().to_string())
                    .collect()
            })
            .collect()
    }
}

fn bench_tools(cwd: &Path, arguments: &[&dyn AsRef<OsStr>]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_bench-tools"));
    command
        .current_dir(cwd)
        .stdin(Stdio::null())
        .args(arguments);
    command
}

fn succeeded(output: &Output) {
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn digest(path: &Path) -> String {
    hex_digest(&fs::read(path).unwrap())
}

fn quote(text: &str) -> String {
    pyjson::dumps(&Value::Str(text.to_string()), &Dumps::default())
}

fn quoted(path: &Path) -> String {
    quote(path.to_str().unwrap())
}

fn parse(text: &[u8]) -> Value {
    pyjson::loads(std::str::from_utf8(text).unwrap(), Duplicates::KeepLast).unwrap()
}

fn read(path: &Path) -> Value {
    parse(&fs::read(path).unwrap())
}

/// Writes the JSON text as `json.dumps(value, indent=2)` does, with a final newline.
fn write(path: &Path, json: &str) {
    let indented = Dumps {
        indent: Some(2),
        ..Dumps::default()
    };
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        path,
        pyjson::dumps(&parse(json.as_bytes()), &indented) + "\n",
    )
    .unwrap();
}

fn get<'a>(value: &'a Value, key: &str) -> &'a Value {
    value
        .as_object()
        .and_then(|object| object.get(key))
        .unwrap_or_else(|| panic!("no {key} in {}", value.py_repr()))
}

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    get(value, key).as_str().unwrap()
}

fn relative(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string()
}

fn files_under(directory: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            found.extend(files_under(&path));
        } else if path.is_file() {
            found.push(path);
        }
    }
    found.sort();
    found
}

fn tar_gz(name: &str, data: &[u8]) -> Vec<u8> {
    let mut bundle = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::default()));
    let mut header = tar::Header::new_ustar();
    header.set_size(data.len() as u64);
    header.set_mode(0o644);
    bundle.append_data(&mut header, name, data).unwrap();
    bundle.into_inner().unwrap().finish().unwrap()
}

fn tar_gz_members(path: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut archive = tar::Archive::new(GzDecoder::new(fs::File::open(path).unwrap()));
    archive
        .entries()
        .unwrap()
        .map(|entry| {
            let mut entry = entry.unwrap();
            let name = entry.path().unwrap().to_str().unwrap().to_string();
            let mut data = Vec::new();
            entry.read_to_end(&mut data).unwrap();
            (name, data)
        })
        .collect()
}

#[test]
fn success_failed_replacement_and_grading_survive_origin_removal() {
    let scratch = Scratch::new("survive");
    let base = &scratch.base;
    let (origin, cold) = (base.join("origin"), base.join("cold"));
    fs::create_dir(&origin).unwrap();
    fs::create_dir(&cold).unwrap();
    let run = origin.join("bench/runs/example");
    let dispositions = [
        ("att-001", "valid completed"),
        ("att-002", "failed: fixture"),
        ("att-003", "valid completed"),
    ];
    for (name, disposition) in dispositions {
        let attempt = run.join("attempts").join(name);
        fs::create_dir_all(&attempt).unwrap();
        let archive = base.join(format!("{name}.tar.gz"));
        let session = format!("session {name}");
        fs::write(
            &archive,
            tar_gz("home/.codex/sessions/session.jsonl", session.as_bytes()),
        )
        .unwrap();
        let replaces = if name == "att-003" {
            r#""att-002""#
        } else {
            "null"
        };
        write(
            &attempt.join("attempt.json"),
            &format!(
                r#"{{"attempt_id": "{name}", "disposition": "{disposition}", "replaces": {replaces},
                "usage": {{"usd": 1.25}}, "transcript_archive": {{"path": {}, "sha256": "{}",
                "restoration_check": "passed"}}}}"#,
                quoted(&archive),
                digest(&archive)
            ),
        );
        write(&attempt.join("normalized.json"), r#"{"findings": []}"#);
    }
    succeeded(&scratch.collect(None, &origin, &run));
    let transcript_entries = read(&run.join("transcripts.json"));

    let native_root = base.join("native");
    fs::create_dir_all(native_root.join("run/scratch")).unwrap();
    fs::write(native_root.join("review.json"), "{}").unwrap();
    let probe = native_root.join("run/scratch/probe.sh");
    fs::write(&probe, "#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(&probe, fs::Permissions::from_mode(0o755)).unwrap();
    let index = base.join("native-artifacts.json");
    let indexed: Vec<String> = files_under(&native_root)
        .iter()
        .map(|path| {
            format!(
                r#"{{"path": {}, "bytes": {}, "sha256": "{}"}}"#,
                quote(&relative(path, &native_root)),
                fs::metadata(path).unwrap().len(),
                digest(path)
            )
        })
        .collect();
    write(
        &index,
        &format!(
            r#"{{"root": {}, "files": [{}]}}"#,
            quoted(&native_root),
            indexed.join(", ")
        ),
    );
    let native_out = run.join("attempts/att-001");
    let record_path = native_out.join("attempt.json");
    let Value::Object(mut record) = read(&record_path) else {
        panic!("attempt.json is not an object");
    };
    fs::remove_file(&record_path).unwrap();
    let filed = scratch.native(&[
        &"file",
        &native_root,
        &index,
        &native_out,
        &"--native-relative",
        &"review.json",
    ]);
    succeeded(&filed);
    record.insert(
        "native_artifact_storage".into(),
        get(&parse(&filed.stdout), "pin").clone(),
    );
    fs::write(
        &record_path,
        pyjson::dumps(&Value::Object(record), &Dumps::default()),
    )
    .unwrap();

    let queue = base.join("queue");
    let grading = queue.join("run/target/attempt-1");
    let work = grading.join("work");
    fs::create_dir_all(work.join("home/.codex")).unwrap();
    fs::write(
        work.join("home/.codex/config.toml"),
        "model = \"fixture\"\n",
    )
    .unwrap();
    write(
        &work.join("dispatch.json"),
        r#"{"exit_code": 0, "usage": {"high": 2.0}}"#,
    );
    write(&work.join("verdicts.json"), r#"{"verdicts": []}"#);
    write(&grading.join("reservation.json"), r#"{"maxBudgetUsd": 6}"#);
    let archived = bench_tools(
        &origin,
        &[
            &"regrade",
            &"archive-attempt",
            &grading,
            &origin.join("bench/regrading/fixture"),
            &"--root",
            &origin,
        ],
    )
    .output()
    .unwrap();
    succeeded(&archived);
    let grading_ref = parse(&archived.stdout);
    let grading_archive = text(
        get(&read(&origin.join(text(&grading_ref, "path"))), "archive"),
        "path",
    )
    .to_string();

    let files = files_under(&origin);
    let frozen: Vec<(String, Vec<u8>)> = files
        .iter()
        .map(|path| (relative(path, &origin), fs::read(path).unwrap()))
        .collect();
    let imported: Vec<String> = frozen
        .iter()
        .filter(|(name, _)| !name.starts_with("artifacts/transcripts/"))
        .map(|(name, data)| {
            format!(
                r#"{{"path": {}, "sha256": "{}"}}"#,
                quote(name),
                hex_digest(data)
            )
        })
        .collect();
    let transcripts: Vec<String> = transcript_entries
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| pyjson::dumps(entry, &Dumps::default()))
        .chain([
            r#"{"path": "artifacts/transcripts/absent.tar.gz", "status": "missing"}"#.to_string(),
        ])
        .collect();
    write(
        &origin.join("bench/import-manifest.json"),
        &format!(
            r#"{{"files": [{}], "transcripts": [{}]}}"#,
            imported.join(", "),
            transcripts.join(", ")
        ),
    );
    let archives: Vec<&PathBuf> = files
        .iter()
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "gz" || extension == "zip")
        })
        .collect();
    let selected: Vec<String> = archives
        .iter()
        .map(|path| {
            format!(
                r#"{{"source": {}, "path": {}, "kind": "evidence"}}"#,
                quoted(path),
                quote(&relative(path, &origin))
            )
        })
        .collect();
    let remote = Release::new(base);
    let manifest_path = origin.join("bench/evidence/manifests/fixture.json");
    scratch.publish(
        &remote,
        &format!(
            r#"{{"repository": "{REPOSITORY}", "tag": "{TAG}",
            "subjects": ["att-001", "att-002", "att-003", "grading"], "files": [{}]}}"#,
            selected.join(", ")
        ),
        &manifest_path,
    );
    for path in files_under(&origin) {
        if !archives.contains(&&path) {
            let target = cold.join(path.strip_prefix(&origin).unwrap());
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::copy(&path, &target).unwrap();
        }
    }
    fs::remove_dir_all(&origin).unwrap();
    fs::remove_dir_all(&queue).unwrap();
    fs::remove_dir_all(&native_root).unwrap();
    fs::remove_dir_all(base.join("packed")).unwrap();
    for (name, _) in dispositions {
        fs::remove_file(base.join(format!("{name}.tar.gz"))).unwrap();
    }

    succeeded(&scratch.collect(Some(&remote), &cold, &cold.join("bench/runs/example")));
    assert!(!cold.join(&grading_archive).exists());
    succeeded(&scratch.run(Some(&remote), &cold, &[&"import-benchmark", &"--check"]));
    let native_cold = cold.join(native_out.strip_prefix(&origin).unwrap());
    let restored = base.join("restored-native");
    succeeded(&scratch.native(&[&"verify", &native_cold]));
    succeeded(&scratch.native(&[&"restore", &native_cold, &"--out", &restored]));
    assert_eq!(
        fs::metadata(restored.join("run/scratch/probe.sh"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o755
    );
    assert_eq!(
        fs::read_to_string(restored.join("review.json")).unwrap(),
        "{}"
    );
    let archive = &transcript_entries.as_array().unwrap()[0];
    let (archive_path, archive_sha) = (text(archive, "path"), text(archive, "sha256"));
    fs::remove_file(cold.join(archive_path)).unwrap();
    succeeded(&scratch.run(
        Some(&remote),
        base,
        &[
            &"evidence",
            &"resolve",
            &archive_path,
            &"--sha256",
            &archive_sha,
            &"--root",
            &cold,
        ],
    ));
    assert_eq!(digest(&cold.join(archive_path)), archive_sha);
    succeeded(&scratch.run(
        Some(&remote),
        base,
        &[
            &"evidence",
            &"fetch",
            &"--manifest",
            &cold.join(manifest_path.strip_prefix(&origin).unwrap()),
            &"--root",
            &cold,
            &"--path",
            &grading_archive,
        ],
    ));
    let members = tar_gz_members(&cold.join(&grading_archive));
    assert_eq!(
        members["work/home/.codex/config.toml"],
        b"model = \"fixture\"\n"
    );
    let dispatch = parse(&members["work/dispatch.json"]);
    assert!(matches!(get(get(&dispatch, "usage"), "high"), Value::Float(high) if *high == 2.0));
    for (name, data) in &frozen {
        assert_eq!(&fs::read(cold.join(name)).unwrap(), data, "{name}");
    }
    let manifest = read(&cold.join("bench/import-manifest.json"));
    let last = get(&manifest, "transcripts").as_array().unwrap().last();
    assert_eq!(text(last.unwrap(), "status"), "missing");
    assert!(
        remote
            .calls()
            .iter()
            .all(|call| call.join(" ").contains(REPOSITORY))
    );
}

#[test]
fn records_naming_their_origin_checkout_restore_in_a_checkout_at_another_path() {
    let scratch = Scratch::new("another-path");
    let base = &scratch.base;
    let (origin, cold) = (base.join("origin"), base.join("cold"));
    let run = origin.join("bench/runs/example");
    let records = [
        ("attempts/att-001", "valid completed"),
        ("attempts/att-002", "failed: fixture"),
        ("attempts/att-003", "valid completed"),
        ("probes/att-001", "valid completed"),
    ];
    for (name, disposition) in records {
        let (layer, attempt_id) = name.split_once('/').unwrap();
        let directory = if layer == "attempts" {
            "example"
        } else {
            "example-probe"
        };
        let archive = origin
            .join("artifacts/transcripts")
            .join(directory)
            .join(format!("{attempt_id}.tar.gz"));
        fs::create_dir_all(archive.parent().unwrap()).unwrap();
        fs::write(&archive, format!("transcript of {name}")).unwrap();
        let predecessor = if name == "attempts/att-003" {
            r#""att-002""#
        } else {
            "null"
        };
        write(
            &run.join(name).join("attempt.json"),
            &format!(
                r#"{{"run_id": "example", "attempt_id": "{attempt_id}", "disposition": "{disposition}",
                "cell": {{"target": "task"}}, "predecessor": {predecessor},
                "usage": {{"metering_status": "complete"}},
                "observed": {{"tree_identity_before": "same", "tree_identity_after": "same"}},
                "transcript_archive": {{"path": {}, "sha256": "{}", "restoration_check": "passed"}}}}"#,
                quoted(&archive),
                digest(&archive)
            ),
        );
        fs::write(run.join(name).join("normalized.json"), "{}").unwrap();
        fs::write(run.join(name).join("usage-requests.jsonl"), "{}\n").unwrap();
    }
    succeeded(&scratch.collect(None, &origin, &run));
    let mapping = fs::read(run.join("transcripts.json")).unwrap();
    let entries: Vec<(String, String)> = read(&run.join("transcripts.json"))
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            (
                text(entry, "path").to_string(),
                text(entry, "sha256").to_string(),
            )
        })
        .collect();
    assert_eq!(
        entries.iter().map(|(path, _)| path).collect::<Vec<_>>(),
        [
            "artifacts/transcripts/example/att-001.tar.gz",
            "artifacts/transcripts/example/att-002.tar.gz",
            "artifacts/transcripts/example/att-003.tar.gz",
            "artifacts/transcripts/example-probe/att-001.tar.gz",
        ]
    );
    let remote = Release::new(base);
    let selected: Vec<String> = entries
        .iter()
        .map(|(path, _)| {
            format!(
                r#"{{"source": {}, "path": {}, "kind": "evidence"}}"#,
                quoted(&origin.join(path)),
                quote(path)
            )
        })
        .collect();
    let subjects: Vec<String> = records.iter().map(|(name, _)| quote(name)).collect();
    scratch.publish(
        &remote,
        &format!(
            r#"{{"repository": "{REPOSITORY}", "tag": "{TAG}", "subjects": [{}], "files": [{}]}}"#,
            subjects.join(", "),
            selected.join(", ")
        ),
        &origin.join("bench/evidence/manifests/fixture.json"),
    );
    for path in files_under(&origin.join("bench")) {
        let target = cold.join(path.strip_prefix(&origin).unwrap());
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::copy(&path, &target).unwrap();
    }
    fs::remove_dir_all(&origin).unwrap();
    fs::remove_dir_all(base.join("packed")).unwrap();
    let cold_run = cold.join("bench/runs/example");
    let forget_local_copies = || {
        for name in ["artifacts", ".cache"] {
            let _ = fs::remove_dir_all(cold.join(name));
        }
    };
    let (second_path, second_sha) = &entries[1];

    succeeded(&scratch.collect(Some(&remote), &cold, &cold_run));
    assert_eq!(
        fs::read(cold_run.join("transcripts.json")).unwrap(),
        mapping
    );
    for (path, sha) in &entries {
        assert_eq!(&digest(&cold.join(path)), sha);
    }
    forget_local_copies();
    succeeded(&scratch.run(
        Some(&remote),
        base,
        &[
            &"evidence",
            &"resolve",
            second_path,
            &"--sha256",
            second_sha,
            &"--root",
            &cold,
        ],
    ));
    assert_eq!(&digest(&cold.join(second_path)), second_sha);

    forget_local_copies();
    let asset = remote.assets().remove(0);
    let published = fs::read(&asset).unwrap();
    fs::write(&asset, vec![0; published.len()]).unwrap();
    let done = scratch.collect(Some(&remote), &cold, &cold_run);
    assert_eq!(done.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&done.stderr)
            .starts_with("collect_run.py: missing or changed evidence: ")
    );
    let done = scratch.run(
        Some(&remote),
        base,
        &[
            &"evidence",
            &"resolve",
            second_path,
            &"--sha256",
            second_sha,
            &"--root",
            &cold,
        ],
    );
    assert_eq!(
        (done.status.code(), done.stderr.as_slice()),
        (Some(1), &b""[..])
    );
    assert!(
        String::from_utf8_lossy(&done.stdout)
            .starts_with("evidence: missing or changed evidence: ")
    );
    fs::write(&asset, &published).unwrap();
    fs::remove_dir_all(remote.state.join("assets")).unwrap();
    let done = scratch.collect(Some(&remote), &cold, &cold_run);
    assert_eq!(
        (done.status.code(), String::from_utf8_lossy(&done.stderr)),
        (
            Some(1),
            "collect_run.py: published release or asset identity changed\n".into()
        )
    );
    let done = scratch.run(
        Some(&remote),
        base,
        &[&"evidence", &"resolve", second_path, &"--root", &cold],
    );
    assert_eq!(
        String::from_utf8_lossy(&done.stdout),
        "evidence: published release or asset identity changed\n"
    );
    assert_eq!(
        fs::read(cold_run.join("transcripts.json")).unwrap(),
        mapping
    );
    assert!(!cold.join("artifacts").exists());
}

#[test]
fn archive_stored_under_a_path_no_record_names_is_mapped_by_its_hash() {
    let scratch = Scratch::new("hash-mapped");
    let base = &scratch.base;
    let root = base.join("checkout");
    let payloads: [(&str, &[u8]); 3] = [
        (
            "artifacts/transcripts/example-probes/att-009.tar.gz",
            b"stored once",
        ),
        ("artifacts/transcripts/other/first.tar.gz", b"stored twice"),
        ("artifacts/transcripts/other/second.tar.gz", b"stored twice"),
    ];
    let mut selected = Vec::new();
    for (index, (name, data)) in payloads.iter().enumerate() {
        let source = base.join(index.to_string());
        fs::write(&source, data).unwrap();
        selected.push(format!(
            r#"{{"source": {}, "path": {}, "kind": "evidence"}}"#,
            quoted(&source),
            quote(name)
        ));
    }
    let remote = Release::new(base);
    scratch.publish(
        &remote,
        &format!(
            r#"{{"repository": "{REPOSITORY}", "tag": "{TAG}", "subjects": ["example"], "files": [{}]}}"#,
            selected.join(", ")
        ),
        &root.join("bench/evidence/manifests/fixture.json"),
    );
    let run = root.join("bench/runs/example");
    let record = |name: &str, data: &[u8], recorded: &str| {
        write(
            &run.join(name).join("attempt.json"),
            &format!(
                r#"{{"transcript_archive": {{"path": {}, "sha256": "{}"}}}}"#,
                quote(recorded),
                hex_digest(data)
            ),
        );
    };
    let gone = "~/gone/cache/example-probe/att-001.tar.gz";

    record("probes/att-001", b"stored once", gone);
    let held = "artifacts/transcripts/held/att-002.tar.gz";
    fs::create_dir_all(root.join(held).parent().unwrap()).unwrap();
    fs::write(root.join(held), b"stored once").unwrap();
    record(
        "probes/att-002",
        b"stored once",
        &format!("/gone/checkout/{held}"),
    );
    succeeded(&scratch.collect(Some(&remote), &root, &run));
    let mapped: Vec<String> = read(&run.join("transcripts.json"))
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| text(entry, "path").to_string())
        .collect();
    assert_eq!(
        mapped,
        ["artifacts/transcripts/example-probes/att-009.tar.gz", held]
    );
    assert_eq!(
        fs::read(root.join("artifacts/transcripts/example-probes/att-009.tar.gz")).unwrap(),
        b"stored once"
    );
    record("attempts/att-001", b"stored twice", gone);
    let done = scratch.collect(Some(&remote), &root, &run);
    assert_eq!(done.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&done.stderr).contains(
        "collect_run.py: Transcript archive unavailable: artifacts/transcripts/example/att-001.tar.gz"
    ));
}
