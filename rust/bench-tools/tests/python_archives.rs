//! Python's `tarfile`, `gzip` and `zipfile`, through the retained `evidence_store.py` and `native_artifacts.py`, read
//! what `bench_tools::archive` writes, and the reverse. Delete this file with those libraries (#159).

use bench_tools::archive::{self, Limits, PackageWriter, ScratchWriter};
use bench_tools::pyjson::{self, Duplicates, Value};
use bench_tools::sha256;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

const PYTHON: &str = r#"
import gzip, json, os, sys, zipfile
from pathlib import Path
sys.path.insert(0, sys.argv[1])
import evidence_store, native_artifacts
command, *rest = sys.argv[2:]
if command == "restore":
    archive, package, destination = rest
    evidence_store.unpack(archive, json.loads(Path(package).read_text()), Path(destination))
elif command == "gunzip":
    Path(rest[1]).write_bytes(gzip.decompress(Path(rest[0]).read_bytes()))
elif command == "list-zip":
    with zipfile.ZipFile(rest[0]) as bundle:
        print(json.dumps([[info.filename, info.compress_type, info.external_attr >> 16, list(info.date_time),
                           bundle.read(info).hex()] for info in bundle.infolist()]))
elif command == "pack":
    selection, out = rest
    print(json.dumps(evidence_store.pack(json.loads(Path(selection).read_text()), out)))
elif command == "file-scratch":
    root, index, out = rest
    native_artifacts.file_artifacts(Path(root), Path(index), Path(out))
"#;

fn scratch(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "bench-tools-python-archives-{label}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn python(arguments: &[&Path]) -> String {
    let tools = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench/tools");
    let output = Command::new("python3")
        .arg("-I")
        .arg("-c")
        .arg(PYTHON)
        .arg(tools)
        .args(arguments)
        .output()
        .expect("python3 runs");
    assert!(
        output.status.success(),
        "python failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn mode_of(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}

fn members() -> Vec<(String, u32, Vec<u8>)> {
    vec![
        ("bench/a.txt".to_string(), 0o644, b"alpha\n".to_vec()),
        (
            format!("bench/{}", "long-".repeat(40)),
            0o755,
            b"long name\n".repeat(300),
        ),
        ("docs/empty".to_string(), 0o600, Vec::new()),
        (
            "artifacts/ü/naïve.json".to_string(),
            0o640,
            b"{}\n".to_vec(),
        ),
    ]
}

fn string(text: &str) -> Value {
    Value::Str(text.to_string())
}

fn int(value: u64) -> Value {
    Value::Int(value.to_string())
}

fn object(entries: Vec<(&str, Value)>) -> Value {
    let mut object = pyjson::Object::default();
    for (key, value) in entries {
        object.insert(key.to_string(), value);
    }
    Value::Object(object)
}

fn dumps(value: &Value) -> String {
    pyjson::dumps(value, &pyjson::Dumps::default())
}

#[test]
fn python_restores_a_rust_package_to_identical_names_bytes_and_modes() {
    let root = scratch("restore");
    let mut writer = PackageWriter::new(Vec::new());
    for (name, mode, data) in members() {
        writer
            .append(
                name.as_bytes(),
                data.len() as u64,
                mode,
                &mut data.as_slice(),
            )
            .unwrap();
    }
    let compressed = writer.finish().unwrap();
    let archive_path = root.join("package.tar.gz");
    std::fs::write(&archive_path, &compressed).unwrap();
    let package = object(vec![
        ("sha256", string(&sha256::hex_digest(&compressed))),
        ("bytes", int(compressed.len() as u64)),
        (
            "members",
            Value::Array(
                members()
                    .iter()
                    .map(|(name, mode, data)| {
                        object(vec![
                            ("path", string(name)),
                            ("sha256", string(&sha256::hex_digest(data))),
                            ("bytes", int(data.len() as u64)),
                            ("mode", int(u64::from(*mode))),
                        ])
                    })
                    .collect(),
            ),
        ),
    ]);
    std::fs::write(root.join("package.json"), dumps(&package)).unwrap();
    python(&[
        Path::new("restore"),
        &archive_path,
        &root.join("package.json"),
        &root.join("restored"),
    ]);
    for (name, mode, data) in members() {
        let path = root.join("restored").join(&name);
        assert_eq!(std::fs::read(&path).unwrap(), data, "{name}");
        assert_eq!(mode_of(&path), mode, "{name}");
    }
    let mut gzip = archive::gzip_writer(Vec::new(), 9);
    gzip.write_all(b"plain gzip\n").unwrap();
    std::fs::write(root.join("plain.gz"), gzip.finish().unwrap()).unwrap();
    python(&[
        Path::new("gunzip"),
        &root.join("plain.gz"),
        &root.join("plain"),
    ]);
    assert_eq!(std::fs::read(root.join("plain")).unwrap(), b"plain gzip\n");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn python_reads_a_rust_scratch_archive_with_its_metadata() {
    let root = scratch("zip");
    let data = b"probe\n".repeat(50);
    let name = sha256::hex_digest(&data);
    let mut writer = ScratchWriter::new(Cursor::new(Vec::new()));
    writer.append(&name, &mut data.as_slice()).unwrap();
    let path = root.join("native-scratch.v1.zip");
    std::fs::write(&path, writer.finish().unwrap().into_inner()).unwrap();
    let listed = python(&[Path::new("list-zip"), &path]);
    let hex: String = data.iter().map(|byte| format!("{byte:02x}")).collect();
    assert_eq!(
        listed.trim(),
        format!("[[\"{name}\", 8, 384, [1980, 1, 1, 0, 0, 0], \"{hex}\"]]")
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn rust_reads_python_packages_and_scratch_archives_to_identical_bytes_and_modes() {
    let root = scratch("python-written");
    let sources = root.join("sources");
    let mut files = Vec::new();
    for (index, (name, mode, data)) in members().into_iter().enumerate() {
        let source = sources.join(index.to_string());
        std::fs::create_dir_all(&sources).unwrap();
        std::fs::write(&source, &data).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&source, std::fs::Permissions::from_mode(mode)).unwrap();
        files.push(object(vec![
            ("source", string(source.to_str().unwrap())),
            ("path", string(&name)),
            ("kind", string("evidence")),
        ]));
    }
    let selection = object(vec![
        ("repository", string("owner/name")),
        ("tag", string("evidence-test")),
        ("subjects", Value::Array(vec![string("test")])),
        ("files", Value::Array(files)),
    ]);
    std::fs::write(root.join("selection.json"), dumps(&selection)).unwrap();
    let out = root.join("packed");
    let manifest = python(&[Path::new("pack"), &root.join("selection.json"), &out]);
    let manifest = pyjson::loads(&manifest, Duplicates::KeepLast).unwrap();
    let package = &manifest
        .as_object()
        .unwrap()
        .get("packages")
        .unwrap()
        .as_array()
        .unwrap()[0];
    let asset = package
        .as_object()
        .unwrap()
        .get("asset")
        .unwrap()
        .as_str()
        .unwrap();
    let limits = Limits {
        members: 4,
        payload: members().iter().map(|(_, _, data)| data.len() as u64).sum(),
    };
    let mut read = Vec::new();
    archive::read_tar_gz(
        std::fs::File::open(out.join(asset)).unwrap(),
        &limits,
        |member, data| {
            let mut bytes = Vec::new();
            data.read_to_end(&mut bytes)?;
            read.push((
                String::from_utf8(member.name.clone()).unwrap(),
                member.mode,
                bytes,
            ));
            Ok(())
        },
    )
    .unwrap();
    let mut expected = members();
    expected.sort();
    assert_eq!(read, expected);

    let artifacts = root.join("ce-review-artifacts");
    let scratch_files = [
        ("run/scratch/a.mjs", b"one\n".as_slice()),
        ("run/scratch/b.mjs", b"two\n"),
    ];
    let mut rows = Vec::new();
    for (name, data) in scratch_files {
        let path = artifacts.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, data).unwrap();
        rows.push(object(vec![
            ("path", string(name)),
            ("sha256", string(&sha256::hex_digest(data))),
            ("bytes", int(data.len() as u64)),
        ]));
    }
    std::fs::write(
        root.join("index.json"),
        dumps(&object(vec![("files", Value::Array(rows))])),
    )
    .unwrap();
    let filed = root.join("filed");
    python(&[
        Path::new("file-scratch"),
        &artifacts,
        &root.join("index.json"),
        &filed,
    ]);
    let limits = Limits {
        members: 2,
        payload: 0,
    };
    let mut names = Vec::new();
    archive::read_zip(
        std::fs::File::open(filed.join("native-scratch.v1.zip")).unwrap(),
        &limits,
        |member, data| {
            let mut bytes = Vec::new();
            data.read_to_end(&mut bytes)?;
            assert_eq!(
                sha256::hex_digest(&bytes).as_bytes(),
                member.name.as_slice()
            );
            names.push(member.name.clone());
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(names.len(), 2);
    std::fs::remove_dir_all(root).unwrap();
}
