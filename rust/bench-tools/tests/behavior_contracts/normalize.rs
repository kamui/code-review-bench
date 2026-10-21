//! What a command left, and the comparisons a case declares on it: archives by their members, digests and
//! archive sizes as labels, evidence manifests and storage references checked against the bytes beside them,
//! volatile fields, the case root and mask patterns.

use crate::corpus::{field, has, list, string};
use crate::{b64, json, pattern, structured};
use bench_tools::pyjson::{Object, Value};
use bench_tools::{pytext, pyvalue, sha256};
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::Path;

pub enum Kind {
    Symlink(Vec<u8>),
    Directory,
    File(Vec<u8>),
    /// A FIFO or other special file, by its permission bits.
    Special(u32),
    Archive {
        format: String,
        members: Value,
    },
}

pub struct Entry {
    pub path: Vec<u8>,
    pub kind: Kind,
    /// Recorded for files and directories when the corpus sets `snapshot_modes`.
    pub mode: Option<u32>,
}

pub struct Observed {
    pub exit: i64,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub filesystem: Vec<Entry>,
    pub inconsistencies: Vec<String>,
}

fn int(value: impl ToString) -> Value {
    Value::Int(value.to_string())
}

impl Entry {
    /// The entry as the corpus writes an expected one.
    pub fn to_value(&self) -> Value {
        let mut object = Object::default();
        object.insert("path_b64".into(), json::text(&b64::encode(&self.path)));
        let kind = match &self.kind {
            Kind::Symlink(_) => "symlink",
            Kind::Directory => "directory",
            Kind::File(_) => "file",
            Kind::Special(_) => "special",
            Kind::Archive { .. } => "archive",
        };
        object.insert("kind".into(), json::text(kind));
        match &self.kind {
            Kind::Symlink(target) => {
                object.insert("target_b64".into(), json::text(&b64::encode(target)))
            }
            Kind::File(data) => {
                object.insert("bytes_b64".into(), json::text(&b64::encode(data)));
                object.insert("sha256".into(), json::text(&sha256::hex_digest(data)));
            }
            Kind::Special(mode) => object.insert("mode".into(), int(mode)),
            Kind::Archive { format, members } => {
                object.insert("format".into(), json::text(format));
                object.insert("members".into(), members.clone());
            }
            Kind::Directory => {}
        }
        if let Some(mode) = self.mode {
            object.insert("mode".into(), int(mode));
        }
        Value::Object(object)
    }
}

pub fn sort_entries(entries: &mut [Entry]) {
    entries.sort_by_cached_key(|entry| b64::encode(&entry.path));
}

/// `fnmatch.fnmatchcase(os.fsdecode(name), pattern)`.
pub fn matches(name: &[u8], pattern: &str) -> Result<bool, String> {
    let pattern: Vec<u32> = pattern.chars().map(u32::from).collect();
    pyvalue::fnmatchcase(&pytext::surrogateescape(name), &pattern).map_err(|error| error.describe())
}

/// Names in the order Python sorts their `os.fsdecode` text.
pub fn sorted<'a>(names: impl IntoIterator<Item = &'a Vec<u8>>) -> Vec<Vec<u8>> {
    let mut names: Vec<Vec<u8>> = names.into_iter().cloned().collect();
    names.sort_by_cached_key(|name| pytext::surrogateescape(name));
    names
}

pub fn replace(data: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
    if from.is_empty() {
        return data.to_vec();
    }
    let mut out = Vec::with_capacity(data.len());
    let mut at = 0;
    while at < data.len() {
        if data[at..].starts_with(from) {
            out.extend_from_slice(to);
            at += from.len();
        } else {
            out.push(data[at]);
            at += 1;
        }
    }
    out
}

/// The case root as written and resolved, longest first, for `mask_root`.
pub fn roots(root: &Path) -> Vec<Vec<u8>> {
    let written = pytext::os_bytes(root.as_os_str()).to_vec();
    let mut roots = vec![written.clone()];
    if let Ok(real) = pytext::realpath(&written, false)
        && real != written
    {
        roots.push(real);
    }
    roots.sort_by_key(|root| std::cmp::Reverse(root.len()));
    roots
}

pub fn mask_root(case: &Value, roots: &[Vec<u8>], mut data: Vec<u8>) -> Vec<u8> {
    if case
        .as_object()
        .and_then(|object| object.get("mask_root"))
        .is_some_and(Value::truthy)
    {
        for root in roots {
            data = replace(&data, root, b"<root>");
        }
    }
    data
}

pub fn mask_patterns(case: &Value, mut data: Vec<u8>) -> Result<Vec<u8>, String> {
    for rule in list(case, "mask_patterns")? {
        data = pattern::substitute(string(rule, "pattern")?, string(rule, "label")?, &data)?;
    }
    Ok(data)
}

/// `str(PurePosixPath(parent of name) / other)`.
fn beside(name: &[u8], other: &str) -> Vec<u8> {
    let joined = if other.starts_with('/') {
        other.as_bytes().to_vec()
    } else {
        let parent = name
            .iter()
            .rposition(|byte| *byte == b'/')
            .map_or(b"." as &[u8], |at| &name[..at]);
        [parent, b"/", other.as_bytes()].concat()
    };
    let absolute = joined.starts_with(b"/");
    let parts: Vec<&[u8]> = joined
        .split(|byte| *byte == b'/')
        .filter(|part| !part.is_empty() && *part != b".")
        .collect();
    let body = parts.join(&b'/');
    match (absolute, body.is_empty()) {
        (true, _) => [b"/" as &[u8], &body].concat(),
        (false, true) => b".".to_vec(),
        (false, false) => body,
    }
}

fn members(raw: &[u8], format: &str) -> Result<Value, String> {
    let io = |error: std::io::Error| error.to_string();
    let mut found = Vec::new();
    match format {
        "tar.gz" => {
            let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(raw));
            for item in archive.entries().map_err(io)? {
                let mut item = item.map_err(io)?;
                let header = item.header();
                let mut kind = header.entry_type().as_byte();
                let mut name = item.path_bytes().into_owned();
                if kind == 0 && name.ends_with(b"/") {
                    kind = b'5';
                }
                if kind == b'5' {
                    while name.len() > 1 && name.ends_with(b"/") {
                        name.pop();
                    }
                }
                let mode = header.mode().map_err(io)?;
                let size = item.size();
                let mut member = Object::default();
                match std::str::from_utf8(&name) {
                    Ok(text) => member.insert("name".into(), json::text(text)),
                    Err(_) => member.insert("name_b64".into(), json::text(&b64::encode(&name))),
                }
                member.insert("type".into(), Value::Str(char::from(kind).to_string()));
                member.insert("size".into(), int(size));
                member.insert("mode".into(), int(mode));
                if matches!(kind, b'0' | 0 | b'7' | b'S') {
                    let mut data = Vec::new();
                    item.read_to_end(&mut data).map_err(io)?;
                    member.insert("sha256".into(), json::text(&sha256::hex_digest(&data)));
                }
                found.push(Value::Object(member));
            }
        }
        "gzip" => {
            let mut data = Vec::new();
            flate2::read::MultiGzDecoder::new(raw)
                .read_to_end(&mut data)
                .map_err(io)?;
            let mut member = Object::default();
            member.insert("size".into(), int(data.len()));
            member.insert("sha256".into(), json::text(&sha256::hex_digest(&data)));
            member.insert("bytes_b64".into(), json::text(&b64::encode(&data)));
            found.push(Value::Object(member));
        }
        "zip" => {
            let mut archive = zip::ZipArchive::new(std::io::Cursor::new(raw))
                .map_err(|error| error.to_string())?;
            for index in 0..archive.len() {
                let mut item = archive.by_index(index).map_err(|error| error.to_string())?;
                let mut member = Object::default();
                member.insert(
                    "name".into(),
                    json::text(&item.name().map_err(|error| error.to_string())?),
                );
                member.insert("size".into(), int(item.size()));
                member.insert("mode".into(), int(item.external_attributes() >> 16));
                let mut data = Vec::new();
                item.read_to_end(&mut data).map_err(io)?;
                member.insert("sha256".into(), json::text(&sha256::hex_digest(&data)));
                found.push(Value::Object(member));
            }
        }
        other => return Err(format!("unknown archive format {other}")),
    }
    Ok(Value::Array(found))
}

fn keyword(case: &Value) -> bool {
    [
        "archives",
        "digests",
        "manifests",
        "references",
        "volatile",
        "mask_root",
        "mask_patterns",
    ]
    .iter()
    .any(|key| has(case, key))
}

/// Applies the case's declared comparisons to what one command left.
pub fn normalize(case: &Value, root: &Path, observed: Observed) -> Result<Observed, String> {
    if has(case, "structured") {
        return structured::structured(case, root, observed);
    }
    if !keyword(case) {
        return Ok(observed);
    }
    let Observed {
        exit,
        stdout,
        stderr,
        filesystem: mut entries,
        mut inconsistencies,
    } = observed;
    let mut raw: HashMap<Vec<u8>, Vec<u8>> = entries
        .iter()
        .filter_map(|entry| match &entry.kind {
            Kind::File(data) => Some((entry.path.clone(), data.clone())),
            _ => None,
        })
        .collect();
    let names = sorted(raw.keys());
    let mut archives = Vec::new();
    for rule in list(case, "archives")? {
        let format = string(rule, "format")?;
        for name in &names {
            if matches(name, string(rule, "glob")?)? {
                let members = match members(&raw[name], format) {
                    Ok(members) => members,
                    Err(error) => {
                        inconsistencies.push(format!(
                            "{}: unreadable {format} archive: {error}",
                            String::from_utf8_lossy(name)
                        ));
                        Value::Null
                    }
                };
                let order = json::dumps_sorted(&members);
                archives.push((
                    order,
                    pytext::surrogateescape(name),
                    format.to_string(),
                    name.clone(),
                    members,
                ));
            }
        }
    }
    archives
        .sort_by(|left, right| (&left.0, &left.1, &left.2).cmp(&(&right.0, &right.1, &right.2)));
    let mut labels: Vec<(String, Vec<u8>)> = Vec::new();
    let label =
        |labels: &mut Vec<(String, Vec<u8>)>, sha: String, text: Vec<u8>, overwrite: bool| {
            match labels.iter_mut().find(|(known, _)| *known == sha) {
                Some(found) if overwrite => found.1 = text,
                Some(_) => {}
                None => labels.push((sha, text)),
            }
        };
    let mut archived = HashSet::new();
    for (number, (_, _, format, name, members)) in archives.into_iter().enumerate() {
        label(
            &mut labels,
            sha256::hex_digest(&raw[&name]),
            format!("<archive {}>", number + 1).into_bytes(),
            true,
        );
        if let Some(entry) = entries.iter_mut().find(|entry| entry.path == name) {
            entry.kind = Kind::Archive { format, members };
        }
        archived.insert(name);
    }
    let substitute = |labels: &[(String, Vec<u8>)], mut data: Vec<u8>| {
        for (sha, text) in labels {
            data = replace(&data, sha.as_bytes(), text);
        }
        data
    };
    for pattern in list(case, "digests")? {
        let pattern = pattern.as_str().ok_or("a digests glob is a string")?;
        for name in &names {
            if matches(name, pattern)? {
                let text = [
                    b"<sha256 of " as &[u8],
                    &substitute(&labels, name.clone()),
                    b">",
                ]
                .concat();
                label(&mut labels, sha256::hex_digest(&raw[name]), text, false);
            }
        }
    }
    type FileMasks = Vec<(Vec<u8>, Vec<(String, Value)>)>;
    let mut masks: FileMasks = Vec::new();
    let mut mask = |name: &[u8], at: String, label: &str| {
        let position = match masks.iter().position(|(known, _)| known == name) {
            Some(position) => position,
            None => {
                masks.push((name.to_vec(), Vec::new()));
                masks.len() - 1
            }
        };
        let pointers = &mut masks[position].1;
        match pointers.iter_mut().find(|(known, _)| *known == at) {
            Some(found) => found.1 = json::text(label),
            None => pointers.push((at, json::text(label))),
        }
    };
    for name in list(case, "manifests")? {
        let shown = name.as_str().ok_or("a manifest path is a string")?;
        let name = shown.as_bytes();
        let Some(data) = raw.get(name) else {
            continue;
        };
        let packages = match json::value(data).and_then(|value| field(&value, "packages").cloned())
        {
            Ok(packages) => packages,
            Err(error) => {
                inconsistencies.push(format!("{shown}: unreadable evidence manifest: {error}"));
                continue;
            }
        };
        for (index, package) in pyvalue::iterate(&packages)
            .map_err(|error| error.describe())?
            .iter()
            .enumerate()
        {
            let object = package
                .as_object()
                .ok_or("an evidence manifest package is an object")?;
            let get = |key: &str| object.get(key).cloned().unwrap_or(Value::Null);
            let asset = beside(name, &get("asset").py_str());
            let consistent = raw.get(&asset).is_some_and(|bytes| {
                get("sha256").py_eq(&json::text(&sha256::hex_digest(bytes)))
                    && get("bytes").py_eq(&int(bytes.len()))
            });
            if !consistent {
                inconsistencies.push(format!(
                    "{shown}: package {index} names {}, {} and {} bytes, which differ from the archive written beside it",
                    get("asset").py_repr(),
                    get("sha256").py_repr(),
                    get("bytes").py_repr()
                ));
            } else if archived.contains(&asset) {
                mask(name, format!("/packages/{index}/bytes"), "<archive size>");
            }
        }
    }
    for rule in list(case, "references")? {
        let shown = string(rule, "path")?;
        let name = shown.as_bytes();
        let Some(data) = raw.get(name) else {
            continue;
        };
        let references = json::value(data).and_then(|record| {
            let record = record
                .as_object()
                .ok_or("a storage record is an object")?
                .clone();
            list(rule, "fields")?
                .iter()
                .map(|key| {
                    let key = key.as_str().ok_or("a reference field is a string")?;
                    Ok((
                        key.to_string(),
                        record
                            .get(key)
                            .cloned()
                            .ok_or(format!("missing field {key:?}"))?,
                    ))
                })
                .collect::<Result<Vec<_>, String>>()
        });
        let references = match references {
            Ok(references) => references,
            Err(error) => {
                inconsistencies.push(format!("{shown}: unreadable storage reference: {error}"));
                continue;
            }
        };
        for (key, reference) in references {
            let path = reference.as_object().and_then(|object| object.get("path"));
            let target = beside(name, &path.cloned().unwrap_or(Value::Null).py_str());
            let consistent = reference.as_object().is_some_and(|object| {
                raw.get(&target).is_some_and(|bytes| {
                    object
                        .get("sha256")
                        .is_some_and(|sha| sha.py_eq(&json::text(&sha256::hex_digest(bytes))))
                })
            });
            if !consistent {
                inconsistencies.push(format!(
                    "{shown}: {key} names {}, which differs from the file written beside it",
                    reference.py_repr()
                ));
            }
        }
    }
    for rule in list(case, "volatile")? {
        let name = string(rule, "path")?;
        for at in list(rule, "pointers")? {
            mask(
                name.as_bytes(),
                at.as_str()
                    .ok_or("a volatile pointer is a string")?
                    .to_string(),
                "<volatile>",
            );
        }
    }
    for (name, pointers) in masks {
        let Some(data) = raw.get(&name) else {
            continue;
        };
        let wanted: HashSet<String> = pointers.iter().map(|(at, _)| at.clone()).collect();
        let shown = String::from_utf8_lossy(&name).into_owned();
        let spans =
            match json::value(data).and_then(|_| json::spans(data, &wanted, &HashSet::new())) {
                Ok(spans) => spans.values,
                Err(error) => {
                    inconsistencies.push(format!("{shown}: volatile fields unreadable: {error}"));
                    continue;
                }
            };
        let mut missing: Vec<&String> = wanted
            .iter()
            .filter(|at| !spans.contains_key(*at))
            .collect();
        if !missing.is_empty() {
            missing.sort();
            inconsistencies.push(format!("{shown}: missing volatile fields {missing:?}"));
        }
        let replacements: HashMap<String, Value> = pointers.into_iter().collect();
        let mut found: Vec<(&String, &(usize, usize))> = spans.iter().collect();
        found.sort_by_key(|(_, (start, _))| std::cmp::Reverse(*start));
        let mut text = data.clone();
        for (at, (start, end)) in found {
            text.splice(*start..*end, json::dumps(&replacements[at]).into_bytes());
        }
        raw.insert(name, text);
    }
    let roots = roots(root);
    let rewrite = |data: Vec<u8>| substitute(&labels, mask_root(case, &roots, data));
    for entry in &mut entries {
        let name = std::mem::take(&mut entry.path);
        if let Kind::File(data) = &mut entry.kind {
            *data = rewrite(raw.remove(&name).unwrap_or_default());
        }
        entry.path = substitute(&labels, name);
    }
    sort_entries(&mut entries);
    let diagnostic = |data: Vec<u8>| mask_patterns(case, data).map(rewrite);
    Ok(Observed {
        exit,
        stdout: diagnostic(stdout)?,
        stderr: diagnostic(stderr)?,
        filesystem: entries,
        inconsistencies,
    })
}
