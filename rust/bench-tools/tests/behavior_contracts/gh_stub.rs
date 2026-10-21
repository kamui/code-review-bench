//! `--contract-gh-stub ARGS...` stands in for `gh` in cases whose corpus names a state directory. It answers only
//! the release reads, upload and asset download that evidence storage runs, and the paginated GET reads of
//! `upstream collect`. It records every argument list in calls.jsonl, and takes its release, assets, upstream
//! responses and injected faults from that directory, so no case reaches the network. `upstream collect` reads
//! four endpoints at a time, so the stub keeps calls.jsonl sorted when it records one of those reads, and the file
//! is the same whatever order they ran in.

use crate::{b64, json};
use bench_tools::pyjson::{self, Duplicates, Value};
use std::ffi::OsString;
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

struct Exit(i32);

fn fail(message: &str, code: i32) -> Exit {
    eprintln!("{message}");
    Exit(code)
}

pub fn role(args: &[OsString]) -> i32 {
    match run(args) {
        Ok(()) => 0,
        Err(Exit(code)) => code,
    }
}

fn read_json(path: &Path) -> Result<Option<Value>, Exit> {
    match std::fs::read(path) {
        Ok(raw) => {
            let text = String::from_utf8(raw)
                .map_err(|error| fail(&format!("{}: {error}", path.display()), 1))?;
            pyjson::loads(&text, Duplicates::KeepLast)
                .map(Some)
                .map_err(|error| fail(&format!("{}: {error}", path.display()), 1))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(fail(&format!("{}: {error}", path.display()), 1)),
    }
}

fn get<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    value.as_object().and_then(|object| object.get(key))
}

fn int(value: Option<&Value>) -> i64 {
    match value {
        Some(Value::Int(digits)) => digits.parse().unwrap_or(0),
        Some(Value::Bool(true)) => 1,
        _ => 0,
    }
}

/// `urllib.parse.quote(text, safe="")`.
fn quote(text: &str) -> String {
    let mut out = String::new();
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"_.-~".contains(&byte) {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn record(state: &Path, args: &[OsString], upstream_read: bool) -> Result<(), Exit> {
    let io = |error: std::io::Error| fail(&format!("calls.jsonl: {error}"), 1);
    let mut log = std::fs::OpenOptions::new()
        .append(true)
        .read(true)
        .create(true)
        .open(state.join("calls.jsonl"))
        .map_err(io)?;
    log.lock().map_err(io)?;
    let line = json::dumps(&Value::Array(
        args.iter()
            .map(|arg| Value::os_text(arg.as_bytes()))
            .collect(),
    ));
    if upstream_read {
        let mut existing = String::new();
        log.seek(SeekFrom::Start(0)).map_err(io)?;
        log.read_to_string(&mut existing).map_err(io)?;
        let mut lines: Vec<&str> = existing.lines().collect();
        lines.push(&line);
        lines.sort_unstable();
        log.set_len(0).map_err(io)?;
        let text: String = lines.iter().map(|line| format!("{line}\n")).collect();
        log.write_all(text.as_bytes()).map_err(io)?;
    } else {
        log.write_all(format!("{line}\n").as_bytes()).map_err(io)?;
    }
    Ok(())
}

fn upstream(state: &Path, endpoint: &OsString) -> Result<(), Exit> {
    let responses = read_json(&state.join("upstream.json"))?;
    let Some(response) = responses.as_ref().and_then(|responses| {
        responses
            .as_object()
            .and_then(|object| object.get_key(endpoint.as_bytes()))
    }) else {
        return Err(fail("gh: Not Found (HTTP 404)", 1));
    };
    let mut stdout = std::io::stdout();
    let written = if let Some(pages) = get(response, "pages") {
        stdout.write_all(json::dumps(pages).as_bytes())
    } else if let Some(encoded) = get(response, "stdout_b64") {
        let data =
            b64::decode(encoded.as_str().unwrap_or_default()).map_err(|error| fail(&error, 1))?;
        stdout.write_all(&data)
    } else {
        stdout.write_all(
            get(response, "stdout")
                .and_then(Value::as_str)
                .unwrap_or("")
                .as_bytes(),
        )
    };
    let _ = written.and_then(|()| stdout.flush());
    eprint!(
        "{}",
        get(response, "stderr")
            .and_then(Value::as_str)
            .unwrap_or("")
    );
    match i32::try_from(int(get(response, "exit"))) {
        Ok(0) | Err(_) => Ok(()),
        Ok(code) => Err(Exit(code)),
    }
}

struct Asset {
    id: i64,
    name: OsString,
    size: u64,
}

fn assets(release: &Value, stored: &Path) -> Vec<Asset> {
    let mut found: Vec<Asset> = (1..=int(get(release, "filler_assets")))
        .map(|n| Asset {
            id: n,
            name: format!("filler-{n}.bin").into(),
            size: 1,
        })
        .collect();
    if stored.is_dir() {
        let mut directories: Vec<(i64, PathBuf)> = std::fs::read_dir(stored)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|entry| {
                let id = entry.file_name().to_str()?.parse().ok()?;
                Some((id, entry.path()))
            })
            .collect();
        directories.sort_by_key(|(id, _)| *id);
        for (id, directory) in directories {
            let mut files: Vec<_> = std::fs::read_dir(&directory)
                .into_iter()
                .flatten()
                .flatten()
                .collect();
            files.sort_by_key(|entry| entry.file_name());
            for file in files {
                found.push(Asset {
                    id,
                    name: file.file_name(),
                    size: file.metadata().map(|info| info.len()).unwrap_or(0),
                });
            }
        }
    }
    found
}

fn asset_value(asset: &Asset) -> Value {
    let mut object = pyjson::Object::default();
    object.insert("id".into(), Value::Int(asset.id.to_string()));
    object.insert("name".into(), Value::os_text(asset.name.as_bytes()));
    object.insert("size".into(), Value::Int(asset.size.to_string()));
    Value::Object(object)
}

fn run(args: &[OsString]) -> Result<(), Exit> {
    let state = PathBuf::from(
        std::env::var_os("CONTRACT_GH_STATE")
            .ok_or_else(|| fail("CONTRACT_GH_STATE is not set", 1))?,
    );
    std::fs::create_dir_all(&state)
        .map_err(|error| fail(&format!("{}: {error}", state.display()), 1))?;
    let words: Vec<&[u8]> = args.iter().map(|arg| arg.as_bytes()).collect();
    let upstream_read = words.len() == 6
        && words[..5]
            == [
                b"api" as &[u8],
                b"--method",
                b"GET",
                b"--paginate",
                b"--slurp",
            ];
    record(&state, args, upstream_read)?;
    if upstream_read {
        return upstream(&state, &args[5]);
    }
    let Some(release) = read_json(&state.join("release.json"))? else {
        return Err(fail("HTTP 404: Not Found", 1));
    };
    let text = |key: &str| get(&release, key).map(Value::py_str).unwrap_or_default();
    let (repository, tag, id) = (text("repository"), text("tag"), text("id"));
    let faults: Vec<String> = get(&release, "faults")
        .and_then(Value::as_array)
        .map(|faults| faults.iter().map(Value::py_str).collect())
        .unwrap_or_default();
    let fault = |name: &str| faults.iter().any(|fault| fault == name);
    let routes = format!("repos/{repository}/releases/");
    let stored = state.join("assets");
    let filler = int(get(&release, "filler_assets"));
    let tag_read = format!("{routes}tags/{}", quote(&tag));
    let listing = format!("{routes}{id}/assets?per_page=100");
    let mut stdout = std::io::stdout();
    if words == [b"api" as &[u8], tag_read.as_bytes()] {
        let mut object = pyjson::Object::default();
        object.insert(
            "id".into(),
            get(&release, "id").cloned().unwrap_or(Value::Null),
        );
        object.insert(
            "tag_name".into(),
            get(&release, "tag").cloned().unwrap_or(Value::Null),
        );
        object.insert(
            "draft".into(),
            get(&release, "draft")
                .cloned()
                .unwrap_or(Value::Bool(false)),
        );
        println!("{}", json::dumps(&Value::Object(object)));
    } else if words
        == [
            b"api" as &[u8],
            b"--paginate",
            b"--slurp",
            listing.as_bytes(),
        ]
    {
        let listed: Vec<Value> = assets(&release, &stored).iter().map(asset_value).collect();
        let mut pages: Vec<Value> = listed
            .chunks(100)
            .map(|page| Value::Array(page.to_vec()))
            .collect();
        if pages.is_empty() {
            pages.push(Value::Array(Vec::new()));
        }
        println!("{}", json::dumps(&Value::Array(pages)));
    } else if words.len() == 6
        && words[..2] == [b"release" as &[u8], b"upload"]
        && words[2] == tag.as_bytes()
        && words[4..] == [b"--repo" as &[u8], repository.as_bytes()]
    {
        let source = Path::new(&args[3]);
        let data = std::fs::read(source)
            .map_err(|error| fail(&format!("{}: {error}", source.display()), 1))?;
        let name = source.file_name().unwrap_or_default();
        let present = assets(&release, &stored);
        if present.iter().any(|asset| asset.name == name) {
            return Err(fail("asset under the same name already exists", 1));
        }
        if fault("rejected-upload") {
            return Err(fail("HTTP 422: upload rejected", 1));
        }
        let next = present
            .iter()
            .map(|asset| asset.id)
            .chain([100])
            .max()
            .unwrap_or(100)
            + 1;
        let target = stored.join(next.to_string());
        std::fs::create_dir_all(stored.as_path())
            .and_then(|()| std::fs::create_dir(&target))
            .map_err(|error| fail(&format!("{}: {error}", target.display()), 1))?;
        let kept = if fault("partial-upload") {
            &data[..data.len() / 2]
        } else {
            &data[..]
        };
        std::fs::write(target.join(name), kept).map_err(|error| fail(&error.to_string(), 1))?;
        if fault("partial-upload") || fault("lost-upload-response") {
            return Err(fail("connection reset by peer", 1));
        }
    } else if words.len() == 4
        && words[0] == b"api"
        && words[1].starts_with(format!("{routes}assets/").as_bytes())
        && words[2..] == [b"-H" as &[u8], b"Accept: application/octet-stream"]
    {
        let Some(asset) = assets(&release, &stored).into_iter().find(|asset| {
            format!("{routes}assets/{}", asset.id).as_bytes() == words[1] && asset.id > filler
        }) else {
            return Err(fail("HTTP 404: Not Found", 1));
        };
        let path = stored.join(asset.id.to_string()).join(&asset.name);
        let mut data = std::fs::read(&path)
            .map_err(|error| fail(&format!("{}: {error}", path.display()), 1))?;
        if fault("truncated-download") {
            data.truncate(data.len() / 2);
        }
        if fault("oversized-download") {
            data.extend([0; 64]);
        }
        let _ = stdout.write_all(&data).and_then(|()| stdout.flush());
    } else {
        return Err(fail("contract gh stub: unsupported arguments", 2));
    }
    Ok(())
}
