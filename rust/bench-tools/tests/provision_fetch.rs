//! `bench-tools provision fetch`: a target's published archive and git mirror placed in a cache root from a fake
//! release. No network is used.

use bench_tools::archive::PackageWriter;
use bench_tools::evidence::{Disk, Error, Remote, Store, System, Upload};
use bench_tools::provision::{self, Options, Outcome};
use bench_tools::pyjson::{self, Duplicates, Value};
use bench_tools::pytext::PyPath;
use bench_tools::sha256;
use std::fs::File;
use std::io::{self, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::Command;

const ARCHIVE: &[u8] = b"dependency archive bytes";
const ARCHIVES: &str = "artifacts/reproducibility/archives";
const MIRRORS: &str = "artifacts/reproducibility/mirrors";

struct Release {
    package: Vec<u8>,
    downloads: u32,
}

impl Remote for Release {
    fn release(&mut self, _repository: &str, _tag: &str) -> Result<Value, Error> {
        let sha = sha256::hex_digest(&self.package);
        let text = format!(
            r#"{{"id": 7, "draft": false, "assets": [{{"id": 101, "name": "{sha}.tar.gz", "size": {}}}]}}"#,
            self.package.len()
        );
        Ok(pyjson::loads(&text, Duplicates::KeepLast).unwrap())
    }

    fn download(
        &mut self,
        _repository: &str,
        _asset_id: &str,
        destination: &mut dyn Write,
        _limit: u64,
    ) -> Result<(), Error> {
        self.downloads += 1;
        destination.write_all(&self.package).unwrap();
        Ok(())
    }

    fn upload(&mut self, _: &str, _: &str, _: &PyPath) -> Result<Upload, Error> {
        unreachable!("fetching never uploads")
    }
}

/// The host filesystem, except that hard links fail as they do across filesystems.
struct NoLinks;

impl Disk for NoLinks {
    fn create(&mut self, path: &Path) -> io::Result<File> {
        System.create(path)
    }

    fn sync(&mut self, file: &File, path: &Path) -> io::Result<()> {
        System.sync(file, path)
    }

    fn link(&mut self, from: &Path, to: &Path) -> io::Result<()> {
        if from.to_string_lossy().contains("/checkout/") && to.to_string_lossy().contains("/cache/")
        {
            return Err(io::Error::from_raw_os_error(18));
        }
        System.link(from, to)
    }

    fn sync_dir(&mut self, path: &Path) -> io::Result<()> {
        System.sync_dir(path)
    }
}

fn scratch(label: &str) -> PathBuf {
    let path =
        std::env::temp_dir().join(format!("bench-tools-fetch-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    std::fs::canonicalize(&path).unwrap()
}

fn git(directory: &Path, arguments: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(directory)
        .args([
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(arguments)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {arguments:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

/// A bundle of a repository holding `main` and `review-head`, and the commit `review-head` names.
fn bundle(directory: &Path) -> (Vec<u8>, String) {
    let repository = directory.join("source");
    std::fs::create_dir_all(&repository).unwrap();
    git(&repository, &["init", "-q", "-b", "main"]);
    std::fs::write(repository.join("a.txt"), "one\n").unwrap();
    git(&repository, &["add", "-A"]);
    git(&repository, &["commit", "-q", "-m", "base"]);
    git(&repository, &["checkout", "-q", "-b", "review-head"]);
    std::fs::write(repository.join("a.txt"), "two\n").unwrap();
    git(&repository, &["commit", "-q", "-am", "head"]);
    git(&repository, &["checkout", "-q", "main"]);
    let file = directory.join("source.bundle");
    git(
        &repository,
        &["bundle", "create", "-q", file.to_str().unwrap(), "--all"],
    );
    (
        std::fs::read(file).unwrap(),
        git(&repository, &["rev-parse", "review-head"]),
    )
}

struct Fixture {
    checkout: PathBuf,
    cache: PathBuf,
    target: PathBuf,
    temporary: PathBuf,
    release: Release,
    head: String,
    sha: String,
}

/// A checkout whose one manifest publishes the archive and, when `mirror` is set, a bundle and receipt, a target
/// pinning the archive, and an empty cache root.
fn fixture(label: &str, mirror: bool) -> Fixture {
    let base = scratch(label);
    let (checkout, cache, target) = (
        base.join("checkout"),
        base.join("cache"),
        base.join("t-one"),
    );
    let temporary = base.join("temporary");
    for directory in [&checkout, &cache, &target, &temporary] {
        std::fs::create_dir_all(directory).unwrap();
    }
    let sha = sha256::hex_digest(ARCHIVE);
    let (bundle, head) = bundle(&base);
    let receipt = br#"{"target": "t-one", "ok": true}"#.to_vec();
    let archive_path = format!("{ARCHIVES}/t-one.{sha}.tar.gz");
    let mut members: Vec<(String, Vec<u8>)> = vec![(archive_path, ARCHIVE.to_vec())];
    if mirror {
        members.push((format!("{MIRRORS}/t-one.bundle"), bundle));
        members.push((format!("{MIRRORS}/t-one.json"), receipt));
    }
    let mut writer = PackageWriter::new(Vec::new());
    for (name, data) in &members {
        writer
            .append(name.as_bytes(), data.len() as u64, 0o644, &mut &data[..])
            .unwrap();
    }
    let package = writer.finish().unwrap();
    let package_sha = sha256::hex_digest(&package);
    let listed: Vec<String> = members
        .iter()
        .map(|(name, data)| {
            format!(
                r#"{{"path": "{name}", "sha256": "{}", "bytes": {}, "mode": 420, "kind": "reproducibility"}}"#,
                sha256::hex_digest(data),
                data.len()
            )
        })
        .collect();
    let manifest = format!(
        r#"{{"schema_version": 1, "repository": "o/r", "tag": "evidence-test-v1", "subjects": ["t-one"], "packages": [{{
          "asset": "{package_sha}.tar.gz", "sha256": "{package_sha}", "bytes": {}, "members": [{}],
          "publication": {{"release_id": 7, "asset_id": 101, "sha256": "{package_sha}", "verified_at": "2026-11-01T00:00:00+00:00"}}}}]}}"#,
        package.len(),
        listed.join(", ")
    );
    std::fs::create_dir_all(checkout.join("bench/evidence/manifests")).unwrap();
    std::fs::write(
        checkout.join("bench/evidence/manifests/deps-v1.json"),
        manifest,
    )
    .unwrap();
    pin(&target, &sha);
    Fixture {
        checkout,
        cache,
        target,
        temporary,
        release: Release {
            package,
            downloads: 0,
        },
        head,
        sha,
    }
}

fn pin(target: &Path, sha: &str) {
    let text = format!(
        r#"{{"id": "t-one", "head": "{0}", "merge_base": "{0}", "negative_shas": [], "diff_manifest_sha256": "{0}",
        "local_base_branch": "main",
        "provisioning": {{"cache": {{"kind": "npm"}}, "dependency_identity": [
          {{"name": "cache archive (npm)", "sha256": "{sha}", "location": "archives/t-one.tar.gz"}}]}}}}"#,
        "a".repeat(40)
    );
    std::fs::write(target.join("target.json"), text).unwrap();
}

impl Fixture {
    fn fetch_with(&mut self, disk: &mut dyn Disk) -> Result<Outcome, String> {
        let options = Options {
            target: self.target.clone(),
            cache_root: self.cache.clone(),
            cache_replacements: None,
        };
        let mut store = Store {
            remote: &mut self.release,
            disk,
            now: &|| "2026-11-04T00:00:00+00:00".to_string(),
            temporary: PyPath::new(self.temporary.as_os_str().as_bytes()),
        };
        provision::fetch::run(&mut store, &self.checkout, &options)
    }

    fn fetch(&mut self) -> Outcome {
        self.fetch_with(&mut System).unwrap()
    }

    fn archive(&self) -> PathBuf {
        self.cache
            .join(format!("archives/t-one.{}.tar.gz", self.sha))
    }
}

#[test]
fn it_places_the_archive_by_hash_as_a_hard_link() {
    let mut fixture = fixture("archive", false);
    let outcome = fixture.fetch();
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    assert_eq!(std::fs::read(fixture.archive()).unwrap(), ARCHIVE);
    let restored = fixture
        .checkout
        .join(format!("{ARCHIVES}/t-one.{}.tar.gz", fixture.sha));
    assert_eq!(
        std::fs::metadata(fixture.archive()).unwrap().ino(),
        std::fs::metadata(restored).unwrap().ino()
    );
    assert!(
        outcome.stdout.contains("no published bundle for t-one"),
        "{}",
        outcome.stdout
    );
    assert!(!fixture.cache.join("mirrors").exists());
}

#[test]
fn it_copies_the_archive_when_a_link_is_impossible() {
    let mut fixture = fixture("copy", false);
    let outcome = fixture.fetch_with(&mut NoLinks).unwrap();
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    assert!(outcome.stdout.contains("copied"), "{}", outcome.stdout);
    assert_eq!(std::fs::read(fixture.archive()).unwrap(), ARCHIVE);
    let restored = fixture
        .checkout
        .join(format!("{ARCHIVES}/t-one.{}.tar.gz", fixture.sha));
    assert_ne!(
        std::fs::metadata(fixture.archive()).unwrap().ino(),
        std::fs::metadata(restored).unwrap().ino()
    );
}

#[test]
fn it_refuses_to_overwrite_a_differing_file() {
    let mut fixture = fixture("differs", false);
    std::fs::create_dir_all(fixture.archive().parent().unwrap()).unwrap();
    std::fs::write(fixture.archive(), b"another build").unwrap();
    let outcome = fixture.fetch();
    assert_eq!(outcome.code, 1, "{}", outcome.stdout);
    assert!(
        outcome.stdout.contains("nothing was overwritten"),
        "{}",
        outcome.stdout
    );
    assert_eq!(std::fs::read(fixture.archive()).unwrap(), b"another build");
    assert_eq!(fixture.release.downloads, 0);
}

#[test]
fn a_matching_file_is_left_alone_without_a_download() {
    let mut fixture = fixture("present", false);
    std::fs::create_dir_all(fixture.archive().parent().unwrap()).unwrap();
    std::fs::write(fixture.archive(), ARCHIVE).unwrap();
    let outcome = fixture.fetch();
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    assert!(
        outcome.stdout.contains("already present"),
        "{}",
        outcome.stdout
    );
    assert_eq!(fixture.release.downloads, 0);
}

#[test]
fn an_unpublished_archive_fails_naming_its_hash() {
    let mut fixture = fixture("unpublished", false);
    let missing = "7".repeat(64);
    pin(&fixture.target, &missing);
    let outcome = fixture.fetch();
    assert_eq!(outcome.code, 1);
    assert_eq!(
        outcome.stdout,
        format!(
            "archive: no published member has SHA-256 {missing} for t-one\nmirror: no published bundle for t-one; build it with provision mirror\n"
        )
    );
    assert!(!fixture.cache.join("archives").exists());
}

#[test]
fn it_restores_the_mirror_from_its_bundle_and_drops_the_origin_remote() {
    let mut fixture = fixture("mirror", true);
    let outcome = fixture.fetch();
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    let mirror = fixture.cache.join("mirrors/t-one.git");
    assert_eq!(
        git(&mirror, &["rev-parse", "refs/heads/review-head"]),
        fixture.head
    );
    assert_eq!(git(&mirror, &["symbolic-ref", "HEAD"]), "refs/heads/main");
    assert_eq!(git(&mirror, &["remote"]), "");
    assert_eq!(
        std::fs::read(fixture.cache.join("mirrors/t-one.json")).unwrap(),
        br#"{"target": "t-one", "ok": true}"#
    );
    let leftovers: Vec<_> = std::fs::read_dir(fixture.cache.join("mirrors"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .filter(|name| name.starts_with('.'))
        .collect();
    assert_eq!(leftovers, Vec::<String>::new());

    let downloads = fixture.release.downloads;
    let again = fixture.fetch();
    assert_eq!(again.code, 0, "{}", again.stdout);
    assert!(
        again.stdout.contains("mirror:") && again.stdout.contains("already present"),
        "{}",
        again.stdout
    );
    assert_eq!(fixture.release.downloads, downloads);
}

#[test]
fn it_keeps_an_existing_mirror_receipt_that_differs() {
    let mut fixture = fixture("receipt", true);
    std::fs::create_dir_all(fixture.cache.join("mirrors")).unwrap();
    std::fs::write(fixture.cache.join("mirrors/t-one.json"), b"{}").unwrap();
    let outcome = fixture.fetch();
    assert_eq!(outcome.code, 1, "{}", outcome.stdout);
    assert_eq!(
        std::fs::read(fixture.cache.join("mirrors/t-one.json")).unwrap(),
        b"{}"
    );
    assert!(!fixture.cache.join("mirrors/t-one.git").exists());
}

#[test]
fn a_relative_cache_root_is_refused() {
    let mut fixture = fixture("relative", false);
    fixture.cache = PathBuf::from("relative-cache");
    let Err(message) = fixture.fetch_with(&mut System) else {
        panic!("a relative cache root was accepted");
    };
    assert!(message.contains("absolute"), "{message}");
}
