//! Packing and publication, as `evidence_store.py`'s `pack` and `publish` do them, and the check a contributor runs
//! on a package directory it packed earlier.
//!
//! Unlike the Python, each package and manifest is written to a synced temporary file and hard-linked to its name,
//! never replacing one, and the directory is synced, so `pack` and `publish` outputs survive a host restart once they
//! exit 0, and an interrupted run leaves no manifest.

use super::{
    ASSET_LIMIT, Disk, Member, Package, Result, Store, TempDir, Upload, archive_failure, array,
    checked_file, digest, exists, field, is_symlink, logical_value, make_dirs, object, open,
    os_failure, parent, parents, read, unpack, validate, verify_remote, violation, write_new,
};
use crate::archive::PackageWriter;
use crate::pyjson::{Object, Value};
use crate::pytext::PyPath;
use std::collections::{BTreeSet, HashSet};
use std::io;
use std::os::unix::fs::PermissionsExt;

/// `evidence_store.PART_LIMIT`, the default `--part-bytes`.
pub const PART_LIMIT: u64 = 1024 * 1024 * 1024;
const SELECTION_KEYS: [&str; 4] = ["files", "repository", "subjects", "tag"];
const FILE_KEYS: [&str; 3] = ["kind", "path", "source"];
const CREDENTIALS: [&[u8]; 2] = [b"auth.json", b".credentials.json"];

/// An input a selection names, with the identity it had when it was selected.
#[derive(Clone, Debug)]
pub struct Input {
    pub source: PyPath,
    pub member: Member,
    pub kind: String,
}

/// A checked selection: its inputs and the manifest they go into.
#[derive(Debug)]
pub struct Selected {
    pub manifest: Object,
    pub inputs: Vec<Input>,
    pub part_limit: u64,
}

fn keys(object: &Object) -> BTreeSet<&str> {
    object.iter().map(|(key, _)| key.as_str()).collect()
}

fn name(path: &PyPath) -> &[u8] {
    path.as_bytes()
        .rsplit(|byte| *byte == b'/')
        .next()
        .unwrap_or_default()
}

fn metadata(path: &PyPath) -> Result<std::fs::Metadata> {
    std::fs::metadata(path.to_path_buf()).map_err(|error| os_failure(&error, path.as_bytes()))
}

/// The positive part size below 2 GiB that `part_limit`, an `int()` in decimal digits, names.
fn part_size(part_limit: &str) -> Result<u64> {
    match part_limit.parse::<u64>() {
        Ok(limit) if limit > 0 && limit < ASSET_LIMIT => Ok(limit),
        _ => Err(violation("part size must be positive and below 2 GiB")),
    }
}

/// The first half of `evidence_store.pack`: check the selection and `output`, then hash each input. `cwd` is what
/// `Path.absolute()` joins a relative source to.
pub fn select(
    selection: &Value,
    output: &PyPath,
    part_limit: &str,
    cwd: &PyPath,
) -> Result<Selected> {
    let shape = || violation("selection requires repository, tag, subjects and files");
    let document = selection.as_object().ok_or_else(shape)?;
    if keys(document) != SELECTION_KEYS.into() {
        return Err(shape());
    }
    let files = document
        .get("files")
        .and_then(Value::as_array)
        .ok_or_else(shape)?;
    if exists(output) {
        return Err(violation("pack output must be a new directory"));
    }
    let part_limit = part_size(part_limit)?;
    let mut inputs: Vec<Input> = Vec::new();
    let mut paths = HashSet::new();
    for raw in files {
        let item_shape =
            || violation("each selection requires source, path and evidence/reproducibility kind");
        let item = raw.as_object().ok_or_else(item_shape)?;
        if keys(item) != FILE_KEYS.into() || item.iter().any(|(_, value)| value.as_str().is_none())
        {
            return Err(item_shape());
        }
        let kind = field(item, "kind")?.py_str();
        if kind != "evidence" && kind != "reproducibility" {
            return Err(item_shape());
        }
        let path = logical_value(field(item, "path")?)?.to_string();
        let source = cwd.join(field(item, "source")?.py_str().as_bytes());
        if CREDENTIALS.contains(&name(&source)) {
            return Err(violation(format!(
                "authentication must not be published: {}",
                source.display()
            )));
        }
        if std::iter::once(source.clone())
            .chain(parents(&source))
            .any(|path| is_symlink(&path))
            || !source.is_file()
        {
            return Err(violation(format!(
                "input must be a regular unlinked file: {}",
                source.display()
            )));
        }
        if paths.contains(&path) || metadata(&source)?.len() > part_limit {
            return Err(violation(
                "duplicate logical path or input exceeds part size",
            ));
        }
        paths.insert(path.clone());
        let sha256 = digest(&source)?;
        let found = metadata(&source)?;
        inputs.push(Input {
            member: Member {
                path,
                sha256,
                bytes: found.len(),
                mode: found.permissions().mode() & 0o777,
            },
            source,
            kind,
        });
    }
    if inputs.is_empty() {
        return Err(violation("selection contains no evidence"));
    }
    let manifest: Object = [
        ("schema_version", Value::Int("1".to_string())),
        ("repository", field(document, "repository")?.clone()),
        ("tag", field(document, "tag")?.clone()),
        ("subjects", field(document, "subjects")?.clone()),
        ("packages", Value::Array(Vec::new())),
    ]
    .into_iter()
    .map(|(key, value)| (key.to_string(), value))
    .collect();
    validate(&Value::Object(manifest.clone()), false)?;
    Ok(Selected {
        manifest,
        inputs,
        part_limit,
    })
}

/// Inputs in logical-path order, a part closing before it would exceed the limit.
fn parts(mut inputs: Vec<Input>, part_limit: u64) -> Vec<Vec<Input>> {
    inputs.sort_by(|left, right| left.member.path.cmp(&right.member.path));
    let mut parts: Vec<Vec<Input>> = vec![Vec::new()];
    let mut size = 0u64;
    for input in inputs {
        let current = parts.last_mut().expect("parts starts with one");
        if !current.is_empty() && size + input.member.bytes > part_limit {
            parts.push(Vec::new());
            size = 0;
        }
        size += input.member.bytes;
        parts
            .last_mut()
            .expect("a part was just ensured")
            .push(input);
    }
    parts
}

fn member_value(input: &Input) -> Value {
    let member = &input.member;
    Value::Object(
        [
            ("path", Value::Str(member.path.clone())),
            ("sha256", Value::Str(member.sha256.clone())),
            ("bytes", Value::Int(member.bytes.to_string())),
            ("mode", Value::Int(member.mode.to_string())),
            ("kind", Value::Str(input.kind.clone())),
        ]
        .into_iter()
        .map(|(key, value)| (key.to_string(), value))
        .collect(),
    )
}

/// Write one part's archive in a temporary directory of `output`, restore it there, then link it into `output`
/// under its SHA-256. Returns the package as the manifest indexes it.
fn write_part(disk: &mut dyn Disk, output: &PyPath, part: &[Input]) -> Result<Value> {
    let temp = TempDir::new(output, "tmp")?;
    let archive = temp.path.join(b"package.tar.gz");
    let os_archive = archive.to_path_buf();
    let file = disk
        .create(&os_archive)
        .map_err(|error| os_failure(&error, archive.as_bytes()))?;
    let mut writer = PackageWriter::new(file);
    for input in part {
        let member = &input.member;
        checked_file(
            &input.source,
            &member.sha256,
            member.bytes,
            Some(member.mode),
        )?;
        let mut source = open(&input.source)?;
        writer
            .append(
                member.path.as_bytes(),
                member.bytes,
                member.mode,
                &mut source,
            )
            .map_err(|error| archive_failure(error, &input.source))?;
    }
    let file = writer
        .finish()
        .map_err(|error| archive_failure(error, &archive))?;
    disk.sync(&file, &os_archive)
        .map_err(|error| os_failure(&error, archive.as_bytes()))?;
    drop(file);
    let sha256 = digest(&archive)?;
    let package = Package {
        asset: format!("{sha256}.tar.gz"),
        sha256,
        bytes: metadata(&archive)?.len(),
        members: part.iter().map(|input| input.member.clone()).collect(),
        publication: None,
    };
    unpack(
        disk,
        &archive,
        &package,
        &temp.path.join(b"restored"),
        &HashSet::new(),
    )?;
    let destination = output.join(package.asset.as_bytes());
    match disk.link(&os_archive, &destination.to_path_buf()) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            checked_file(&destination, &package.sha256, package.bytes, None)?;
        }
        Err(error) => return Err(os_failure(&error, destination.as_bytes())),
    }
    Ok(Value::Object(
        [
            ("asset", Value::Str(package.asset.clone())),
            ("sha256", Value::Str(package.sha256.clone())),
            ("bytes", Value::Int(package.bytes.to_string())),
            (
                "members",
                Value::Array(part.iter().map(member_value).collect()),
            ),
            ("publication", Value::Null),
        ]
        .into_iter()
        .map(|(key, value)| (key.to_string(), value))
        .collect(),
    ))
}

/// The second half of `evidence_store.pack`: make `output`, write each part, then the manifest.
pub fn write(disk: &mut dyn Disk, selected: Selected, output: &PyPath) -> Result<Value> {
    make_dirs(&parent(output))?;
    std::fs::create_dir(output.to_path_buf())
        .map_err(|error| os_failure(&error, output.as_bytes()))?;
    let mut manifest = selected.manifest;
    let mut packages = Vec::new();
    for part in parts(selected.inputs, selected.part_limit) {
        packages.push(write_part(disk, output, &part)?);
    }
    manifest.insert("packages".to_string(), Value::Array(packages));
    let manifest = Value::Object(manifest);
    validate(&manifest, false)?;
    // Syncs `output` and each ancestor after the link, which settles the package links too.
    write_new(disk, &output.join(b"manifest.json"), &manifest)?;
    Ok(manifest)
}

/// `evidence_store.pack`: package a selection's inputs under `output`, a new directory, and write its manifest.
pub fn pack(
    disk: &mut dyn Disk,
    selection: &Value,
    output: &PyPath,
    part_limit: &str,
    cwd: &PyPath,
) -> Result<Value> {
    let selected = select(selection, output, part_limit, cwd)?;
    write(disk, selected, output)
}

/// Check a package directory a contributor packed earlier: it holds exactly `manifest.json` and the assets that
/// manifest names, and every package restores into an empty temporary directory.
pub fn check_package(store: &mut Store<'_>, directory: &PyPath) -> Result<()> {
    let document = read(&directory.join(b"manifest.json"))?;
    let manifest = validate(&document, false)?;
    let mut expected: BTreeSet<Vec<u8>> = manifest
        .packages
        .iter()
        .map(|package| package.asset.as_bytes().to_vec())
        .collect();
    expected.insert(b"manifest.json".to_vec());
    let mut found = crate::pyfs::names(directory.as_bytes());
    found.sort();
    if let Some(extra) = found.iter().find(|name| !expected.contains(*name)) {
        return Err(violation(format!(
            "unexpected file in package: {}",
            directory.join(extra).display()
        )));
    }
    for package in &manifest.packages {
        let temp = TempDir::new(&store.temporary, "evidence-check-")?;
        unpack(
            store.disk,
            &directory.join(package.asset.as_bytes()),
            package,
            &temp.path.join(b"restored"),
            &HashSet::new(),
        )?;
    }
    Ok(())
}

fn without(object: &Object, key: &str) -> Object {
    object
        .iter()
        .filter(|(name, _)| name.as_str() != key)
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect()
}

/// `existing` and `manifest` index the same evidence, publication receipts aside.
fn same_evidence(existing: &Value, manifest: &Value) -> Result<bool> {
    let existing = object(existing)?;
    let manifest = object(manifest)?;
    if !Value::Object(without(existing, "packages"))
        .py_eq(&Value::Object(without(manifest, "packages")))
    {
        return Ok(false);
    }
    let unpublished = |packages: &[Value]| -> Result<Vec<Value>> {
        packages
            .iter()
            .map(|package| Ok(Value::Object(without(object(package)?, "publication"))))
            .collect()
    };
    let left = unpublished(array(field(existing, "packages")?)?)?;
    let right = unpublished(array(field(manifest, "packages")?)?)?;
    Ok(Value::Array(left).py_eq(&Value::Array(right)))
}

fn named<'a>(release: &'a Value, asset: &str) -> Result<Vec<&'a Value>> {
    let mut found = Vec::new();
    for candidate in array(field(object(release)?, "assets")?)? {
        if field(object(candidate)?, "name")?.py_eq(&Value::Str(asset.to_string())) {
            found.push(candidate);
        }
    }
    Ok(found)
}

/// `evidence_store.publish`: upload each package of the manifest at `manifest_path` that its release lacks, never
/// replacing an asset, check the release's asset, download and restore it, and only then write the published
/// manifest to `destination`. An existing published manifest for the same evidence is verified, not uploaded again.
pub fn publish(
    store: &mut Store<'_>,
    manifest_path: &PyPath,
    destination: &PyPath,
) -> Result<Value> {
    let mut document = read(manifest_path)?;
    let manifest = validate(&document, false)?;
    if exists(destination) {
        let existing = read(destination)?;
        validate(&existing, true)?;
        if !same_evidence(&existing, &document)? {
            return Err(violation("refusing to replace a published manifest"));
        }
        verify_remote(store, &existing)?;
        return Ok(existing);
    }
    let directory = parent(manifest_path);
    for (index, package) in manifest.packages.iter().enumerate() {
        let archive = directory.join(package.asset.as_bytes());
        checked_file(&archive, &package.sha256, package.bytes, None)?;
        let mut release = store.remote.release(&manifest.repository, &manifest.tag)?;
        if object(&release)?.get("draft").is_some_and(Value::truthy) {
            return Err(violation("draft releases are not shared evidence storage"));
        }
        if named(&release, &package.asset)?.is_empty() {
            if array(field(object(&release)?, "assets")?)?.len() >= 1000 {
                return Err(violation(
                    "release is full; select another evidence release",
                ));
            }
            if let Upload::Failed(error) =
                store
                    .remote
                    .upload(&manifest.repository, &manifest.tag, &archive)?
            {
                // The server may have committed the asset before the response was lost; never upload twice.
                let after = store.remote.release(&manifest.repository, &manifest.tag)?;
                if named(&after, &package.asset)?.is_empty() {
                    return Err(error);
                }
            }
            release = store.remote.release(&manifest.repository, &manifest.tag)?;
        }
        let assets = named(&release, &package.asset)?;
        let size = Value::Int(package.bytes.to_string());
        if assets.len() != 1 || !field(object(assets[0])?, "size")?.py_eq(&size) {
            return Err(violation("release asset identity or size differs"));
        }
        let publication: Object = [
            ("release_id", field(object(&release)?, "id")?.clone()),
            ("asset_id", field(object(assets[0])?, "id")?.clone()),
            ("sha256", Value::Str(package.sha256.clone())),
            ("verified_at", Value::Str((store.now)())),
        ]
        .into_iter()
        .map(|(key, value)| (key.to_string(), value))
        .collect();
        let Value::Object(fields) = &mut document else {
            unreachable!("validate accepted an object");
        };
        let Some(Value::Array(packages)) = fields.get_mut("packages") else {
            unreachable!("validate accepted a package list");
        };
        let Value::Object(entry) = &mut packages[index] else {
            unreachable!("validate accepted each package");
        };
        entry.insert("publication".to_string(), Value::Object(publication));
        let entry = Value::Object(entry.clone());
        let mut single = fields.clone();
        single.insert("packages".to_string(), Value::Array(vec![entry]));
        verify_remote(store, &Value::Object(single))?;
    }
    validate(&document, true)?;
    write_new(store.disk, destination, &document)?;
    Ok(document)
}
