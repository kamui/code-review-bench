use super::*;
use crate::evidence::System;
use crate::evidence::fake::Release;
use crate::evidence::publication::{self, PART_LIMIT};
use std::cell::Cell;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

const PREFIX: &str = "artifacts/workspaces/failed-001";
const MANIFEST: &str = "bench/evidence/manifests/test.json";
const RESTORED: &str = "bench/evidence/receipts/restored.json";
const INVENTORY: &str = "bench/evidence/inventories/test.json";
const PROOF: &str = "bench/evidence/closure-evidence.json";
const PLAN: &str = "bench/evidence/retirements/test.json";
const COMMIT: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const CATEGORIES: [&str; 6] = [
    "inputs",
    "execution",
    "accounting",
    "diagnostics",
    "disposition",
    "lineage",
];

fn py(path: &Path) -> PyPath {
    PyPath::new(path.as_os_str().as_bytes())
}

fn json(text: &str) -> Value {
    pyjson::loads(text, Duplicates::KeepLast).unwrap()
}

fn quoted(text: &str) -> String {
    pyjson::dumps(&Value::Str(text.to_string()), &pyjson::Dumps::default())
}

fn clock() -> String {
    evidence::isoformat(1_792_411_200, 0)
}

fn idle(_: &PyPath) -> R<Vec<u32>> {
    Ok(Vec::new())
}

fn busy(_: &PyPath) -> R<Vec<u32>> {
    Ok(vec![123])
}

fn refusal(result: R<Value>) -> String {
    match result {
        Err(Error::Violation(message)) => message,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

fn failure(result: R<Value>) -> String {
    match result {
        Err(Error::Io(message)) => message,
        other => panic!("expected a failure, got {other:?}"),
    }
}

/// The canonical repository: the checkout's files as last frozen, and which commits the shared branch reaches.
struct Shared {
    files: HashMap<String, Vec<u8>>,
    reachable: bool,
    unpushed: Option<String>,
}

impl Forge for Shared {
    fn get(&mut self, route: &str) -> R<Value> {
        if let Some((_, compared)) = route.split_once("/compare/") {
            let (commit, _) = compared.split_once("...").unwrap();
            let ahead = self.reachable && self.unpushed.as_deref() != Some(commit);
            return Ok(json(if ahead {
                r#"{"status": "ahead"}"#
            } else {
                r#"{"status": "diverged"}"#
            }));
        }
        let (_, name) = route.split_once("/contents/").unwrap();
        let (name, _) = name.split_once('?').unwrap();
        let Some(data) = self.files.get(name) else {
            return Err(Error::Io(format!("gh api {route}: Not Found")));
        };
        let mut blob = sha1_smol::Sha1::new();
        blob.update(format!("blob {}\0", data.len()).as_bytes());
        blob.update(data);
        Ok(json(&format!(
            r#"{{"type": "file", "sha": "{}", "size": {}}}"#,
            blob.digest(),
            data.len()
        )))
    }
}

/// A failed review's closed workspace, its captured evidence published, beside a checkout and an accounting queue.
struct Fixture {
    base: PathBuf,
    repo: PathBuf,
    work: PathBuf,
    queue: PathBuf,
    release: Release,
    shared: Shared,
    kind: &'static str,
    workspace: String,
    investigation: &'static str,
    categories: Vec<&'static str>,
    roots: Vec<String>,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::process::Command::new("chmod")
            .args(["-R", "u+rwx"])
            .arg(&self.base)
            .status();
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

impl Fixture {
    fn new(label: &str) -> Self {
        let base =
            std::env::temp_dir().join(format!("bench-tools-retire-{}-{label}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let base = std::fs::canonicalize(&base).unwrap();
        let [repo, work, queue] = ["repo", "work", "queue"].map(|name| base.join(name));
        for path in [&repo, &work, &queue, &base.join("tmp")] {
            std::fs::create_dir_all(path).unwrap();
        }
        std::fs::write(
            work.join("dispatch.json"),
            r#"{"exit_code":1,"usage":{"high":2.5}}"#,
        )
        .unwrap();
        std::fs::write(work.join("stderr.log"), "failure diagnostics").unwrap();
        let files: Vec<String> = ["dispatch.json", "stderr.log"]
            .iter()
            .map(|name| {
                format!(
                    r#"{{"source": {}, "path": "{PREFIX}/{name}", "kind": "evidence"}}"#,
                    quoted(work.join(name).to_str().unwrap())
                )
            })
            .collect();
        let selection = json(&format!(
            r#"{{"repository": "owner/canonical", "tag": "evidence-v1", "subjects": ["failed-001"], "files": [{}]}}"#,
            files.join(", ")
        ));
        let packed = base.join("packed");
        publication::pack(
            &mut System,
            &selection,
            &py(&packed),
            &PART_LIMIT.to_string(),
            &py(&base),
        )
        .unwrap();
        let mut release = Release::default();
        for name in [MANIFEST, RESTORED, PROOF, PLAN] {
            std::fs::create_dir_all(repo.join(name).parent().unwrap()).unwrap();
        }
        std::fs::create_dir_all(repo.join(INVENTORY).parent().unwrap()).unwrap();
        {
            let mut store = Store {
                remote: &mut release,
                disk: &mut System,
                now: &clock,
                temporary: py(&base.join("tmp")),
            };
            publication::publish(
                &mut store,
                &py(&packed.join("manifest.json")),
                &py(&repo.join(MANIFEST)),
            )
            .unwrap();
            evidence::restoration_receipt(
                &mut store,
                &py(&repo.join(MANIFEST)),
                &py(&repo.join(RESTORED)),
            )
            .unwrap();
        }
        std::fs::write(
            repo.join(PROOF),
            r#"{"disposition": "failed", "replacement": "next-002", "usage": 2.5, "reason": "inspected failure"}"#,
        )
        .unwrap();
        let mut fixture = Self {
            workspace: work.to_str().unwrap().to_string(),
            roots: vec![queue.to_str().unwrap().to_string()],
            base,
            repo,
            work,
            queue,
            release,
            shared: Shared {
                files: HashMap::new(),
                reachable: true,
                unpushed: None,
            },
            kind: "review",
            investigation: "closed",
            categories: CATEGORIES.to_vec(),
        };
        fixture.snapshot();
        fixture
    }

    /// Save the workspace's inventory as captured now, then publish the plan.
    fn snapshot(&mut self) {
        let snapshot = evidence_inventory::inventory(self.work.as_os_str().as_bytes(), true)
            .unwrap_or_else(|error| panic!("{error:?}"));
        self.write_snapshot(&snapshot);
    }

    fn write_snapshot(&mut self, snapshot: &Value) {
        std::fs::write(
            self.repo.join(INVENTORY),
            pyjson::dumps(snapshot, &pyjson::Dumps::default()),
        )
        .unwrap();
        self.save_plan();
    }

    fn save_plan(&mut self) {
        let proof = format!(
            r#"[{{"path": "{PROOF}", "sha256": "{}"}}]"#,
            evidence::digest(&py(&self.repo.join(PROOF))).unwrap()
        );
        let evidence: Vec<String> = self
            .categories
            .iter()
            .map(|category| format!(r#""{category}": {proof}"#))
            .collect();
        let roots: Vec<String> = self.roots.iter().map(|root| quoted(root)).collect();
        std::fs::write(
            self.repo.join(PLAN),
            format!(
                r#"{{"schema_version": 1, "workspace": {}, "kind": "{}", "manifest": "{MANIFEST}", "inventory": "{INVENTORY}", "restoration_receipt": "{RESTORED}", "capture_prefix": "{PREFIX}", "investigation": "{}", "closure_reason": "failure diagnosed and replacement retained", "closed_by": "fixture maintainer", "maintenance_window": true, "accounting_roots": [{}], "evidence": {{{}}}}}"#,
                quoted(&self.workspace),
                self.kind,
                self.investigation,
                roots.join(", "),
                evidence.join(", ")
            ),
        )
        .unwrap();
        self.freeze();
    }

    /// Publish every file in the checkout as it is now.
    fn freeze(&mut self) {
        let mut files = HashMap::new();
        let mut pending = vec![self.repo.clone()];
        while let Some(directory) = pending.pop() {
            for entry in std::fs::read_dir(&directory).unwrap() {
                let path = entry.unwrap().path();
                if path.file_name().unwrap() == ".git" {
                    continue;
                }
                if path.is_dir() {
                    pending.push(path);
                } else {
                    let name = path.strip_prefix(&self.repo).unwrap().to_str().unwrap();
                    files.insert(name.to_string(), std::fs::read(&path).unwrap());
                }
            }
        }
        self.shared.files = files;
    }

    fn receipt(&self) -> PathBuf {
        self.base.join("receipt.json")
    }

    fn completed(&self) -> PathBuf {
        self.base.join("receipt.json.completed.json")
    }

    fn run(&mut self, apply: bool) -> R<Value> {
        self.run_with(apply, &self.receipt(), &idle, &removal::uninterrupted)
    }

    fn run_with(
        &mut self,
        apply: bool,
        receipt: &Path,
        active: &dyn Fn(&PyPath) -> R<Vec<u32>>,
        hook: Hook<'_>,
    ) -> R<Value> {
        let mut effects = Effects {
            store: Store {
                remote: &mut self.release,
                disk: &mut System,
                now: &clock,
                temporary: py(&self.base.join("tmp")),
            },
            forge: &mut self.shared,
            active,
            hook,
        };
        retire(
            &Request {
                root: self.repo.as_os_str().as_bytes(),
                plan: PLAN,
                commit: COMMIT,
                branch: "main",
                receipt: receipt.as_os_str().as_bytes(),
                apply,
            },
            &mut effects,
        )
    }

    /// An attempt in `queue` whose reservation is settled by the workspace's bytes through `work`.
    fn attempt(&self, queue: &Path) -> PathBuf {
        let attempt = queue.join("batches/run/target/attempt-1");
        std::fs::create_dir_all(&attempt).unwrap();
        std::fs::write(attempt.join("reservation.json"), r#"{"maxBudgetUsd": 6}"#).unwrap();
        attempt
    }

    fn dependencies(&self) -> R<Vec<PyPath>> {
        accounting_dependencies(&py(&self.work), &[py(&self.queue)])
    }
}

fn shown(paths: R<Vec<PyPath>>) -> Vec<String> {
    paths.unwrap().iter().map(PyPath::display).collect()
}

#[test]
fn the_schema_matches_the_repository_copy() {
    assert_eq!(
        SCHEMA,
        include_str!("../../../../bench/schema/evidence-retirement.schema.json")
    );
}

#[test]
fn a_dry_run_then_apply_removes_a_closed_failure_and_keeps_accounting_and_receipts() {
    let mut fixture = Fixture::new("apply");
    let shared = fixture.shared.files.clone();
    let preview = fixture.run(false).unwrap();
    assert!(matches!(at(&preview, "applied"), Value::Bool(false)));
    assert!(fixture.work.join("stderr.log").exists());
    assert!(
        at(&evidence::read(&py(&fixture.receipt())).unwrap(), "state")
            .as_str()
            .is_none()
    );

    let again = refusal(fixture.run(true));
    assert_eq!(again, RECEIPT_REFUSAL, "a dry-run receipt is never reused");
    std::fs::remove_file(fixture.receipt()).unwrap();

    let applied = fixture.run(true).unwrap();
    assert!(matches!(at(&applied, "applied"), Value::Bool(true)));
    assert!(matches!(at(&applied, "free_bytes_delta"), Value::Int(_)));
    assert!(at(&applied, "state").as_str().is_none());
    assert!(!fixture.work.exists());
    assert!(fixture.queue.exists() && fixture.repo.join(PROOF).exists());
    assert_eq!(shared, fixture.shared.files);
    let started = evidence::read(&py(&fixture.receipt())).unwrap();
    assert_eq!(at(&started, "state").as_str(), Some(STARTED));
    let completed = evidence::read(&py(&fixture.completed())).unwrap();
    assert!(completed.py_eq(&applied));

    let retried = fixture.run(true).unwrap();
    assert!(
        retried.py_eq(&applied),
        "a retry returns the completed receipt"
    );
    assert!(fixture.run(false).unwrap().py_eq(&applied));
}

#[test]
fn activity_an_open_investigation_and_missing_diagnostics_refuse() {
    let mut fixture = Fixture::new("open");
    let receipt = fixture.receipt();
    let active = refusal(fixture.run_with(true, &receipt, &busy, &removal::uninterrupted));
    assert_eq!(active, "workspace is active: [123]");
    fixture.investigation = "open";
    fixture.save_plan();
    assert!(refusal(fixture.run(true)).starts_with("invalid closure: "));
    fixture.investigation = "closed";
    fixture
        .categories
        .retain(|category| *category != "diagnostics");
    fixture.save_plan();
    assert_eq!(
        refusal(fixture.run(true)),
        "closure omits required evidence categories"
    );
    assert!(fixture.work.join("stderr.log").exists());
    assert!(!receipt.exists());
}

#[test]
fn a_changed_snapshot_and_an_uncaptured_file_refuse() {
    let mut fixture = Fixture::new("changed");
    std::fs::write(fixture.work.join("stderr.log"), "additional diagnostics").unwrap();
    assert_eq!(
        refusal(fixture.run(true)),
        "workspace changed since the complete inventory was captured"
    );
    fixture.snapshot();
    assert_eq!(
        refusal(fixture.run(true)),
        "uncaptured file blocks retirement: stderr.log"
    );
    assert!(fixture.work.join("stderr.log").exists());
}

#[test]
fn an_unreachable_commit_an_unpublished_plan_and_a_replaced_asset_refuse() {
    let mut fixture = Fixture::new("published");
    fixture.shared.reachable = false;
    assert_eq!(
        refusal(fixture.run(true)),
        "commit is not reachable from the shared branch"
    );
    fixture.shared.reachable = true;
    let plan = std::fs::read_to_string(fixture.repo.join(PLAN)).unwrap();
    std::fs::write(
        fixture.repo.join(PLAN),
        plan.replace("failure diagnosed", "changed locally"),
    )
    .unwrap();
    assert_eq!(
        refusal(fixture.run(true)),
        format!("file is not published at {COMMIT}: {PLAN}")
    );
    fixture.save_plan();
    fixture.release.assets[0].0 += 1;
    assert_eq!(
        refusal(fixture.run(true)),
        "published release or asset identity changed"
    );
    assert!(fixture.work.join("stderr.log").exists());
    assert!(!fixture.receipt().exists());
}

#[test]
fn a_bad_pin_refuses() {
    let mut fixture = Fixture::new("pin");
    let receipt = fixture.receipt();
    let mut effects = Effects {
        store: Store {
            remote: &mut fixture.release,
            disk: &mut System,
            now: &clock,
            temporary: py(&fixture.base.join("tmp")),
        },
        forge: &mut fixture.shared,
        active: &idle,
        hook: &removal::uninterrupted,
    };
    for (commit, branch) in [
        ("main", "main"),
        (&COMMIT.to_uppercase(), "main"),
        (COMMIT, ""),
    ] {
        let request = Request {
            root: fixture.repo.as_os_str().as_bytes(),
            plan: PLAN,
            commit,
            branch,
            receipt: receipt.as_os_str().as_bytes(),
            apply: true,
        };
        assert_eq!(
            refusal(retire(&request, &mut effects)),
            "pin a pushed commit and its shared branch"
        );
    }
}

#[test]
fn a_resolved_zero_charge_reservation_still_depends_on_the_workspace() {
    let fixture = Fixture::new("zero");
    let attempt = fixture.attempt(&fixture.queue);
    symlink(&fixture.work, attempt.join("work")).unwrap();
    std::fs::write(
        attempt.join("budget-resolution.json"),
        r#"{"chargeUpperUsd": 0, "evidence": []}"#,
    )
    .unwrap();
    let expected = [attempt.join("reservation.json").display().to_string()];
    std::fs::remove_file(fixture.work.join("dispatch.json")).unwrap();
    assert_eq!(shown(fixture.dependencies()), expected);
    std::fs::write(fixture.work.join("dispatch.json"), "{}").unwrap();
    assert_eq!(shown(fixture.dependencies()), expected);
}

#[test]
fn a_priced_reservation_through_the_workspace_blocks_removal() {
    let mut fixture = Fixture::new("priced");
    let attempt = fixture.attempt(&fixture.queue.clone());
    symlink(&fixture.work, attempt.join("work")).unwrap();
    let refused = refusal(fixture.run(true));
    assert_eq!(
        refused,
        format!(
            "workspace-dependent accounting: ['{}']",
            attempt.join("reservation.json").display()
        )
    );
    assert!(fixture.work.join("dispatch.json").exists());
    assert!(!fixture.receipt().exists());
}

#[test]
fn a_link_at_any_ledger_component_cannot_hide_a_dependency() {
    let mut fixture = Fixture::new("components");
    let parts = ["batches", "run", "target", "attempt-1"];
    for component in ["queue", "batches", "run", "target", "attempt-1"] {
        let queue = fixture.base.join(format!("queue-{component}"));
        let attempt = fixture.attempt(&queue);
        symlink(&fixture.work, attempt.join("work")).unwrap();
        let linked = match parts.iter().position(|part| *part == component) {
            None => queue.clone(),
            Some(index) => parts[..=index]
                .iter()
                .fold(queue.clone(), |path, part| path.join(part)),
        };
        let moved = fixture.base.join(format!("linked-{component}"));
        std::fs::rename(&linked, &moved).unwrap();
        symlink(&moved, &linked).unwrap();
        fixture.roots = vec![queue.display().to_string()];
        fixture.save_plan();
        let refused = refusal(fixture.run(true));
        assert!(
            refused.starts_with("workspace-dependent accounting: "),
            "{component}: {refused}"
        );
        assert!(fixture.work.join("dispatch.json").exists());
    }
}

#[test]
fn an_external_dispatch_link_into_the_workspace_blocks_removal() {
    let mut fixture = Fixture::new("dispatch");
    let base = fixture.base.clone();
    let work = fixture.work.clone();
    let persistent = base.join("persistent-dispatch.json");
    std::fs::copy(work.join("dispatch.json"), &persistent).unwrap();
    let intermediate = base.join("receipt-link");
    symlink(work.join("../persistent-dispatch.json"), &intermediate).unwrap();
    symlink(&base, base.join("parent-alias")).unwrap();
    std::fs::create_dir(base.join("prefix")).unwrap();
    let targets = [
        work.join("dispatch.json"),
        work.join("../persistent-dispatch.json"),
        intermediate,
        base.join("parent-alias/work/dispatch.json"),
        base.join("prefix/../work/../persistent-dispatch.json"),
    ];
    for (index, target) in targets.iter().enumerate() {
        let queue = base.join(format!("receipt-queue-{index}"));
        let attempt = fixture.attempt(&queue);
        std::fs::create_dir(attempt.join("work")).unwrap();
        symlink(target, attempt.join("work/dispatch.json")).unwrap();
        fixture.roots = vec![queue.display().to_string()];
        fixture.save_plan();
        let refused = refusal(fixture.run(true));
        assert!(
            refused.starts_with("workspace-dependent accounting: "),
            "{target:?}: {refused}"
        );
        assert!(work.join("dispatch.json").exists());
    }
}

#[test]
fn a_zero_charge_proof_in_the_workspace_is_a_dependency_and_a_relative_one_refuses() {
    let mut fixture = Fixture::new("proof");
    let attempt = fixture.attempt(&fixture.queue.clone());
    std::fs::create_dir(attempt.join("work")).unwrap();
    let evidence = fixture.work.join("stderr.log");
    std::fs::write(
        attempt.join("budget-resolution.json"),
        format!(
            r#"{{"chargeUpperUsd": 0, "evidence": [{{"path": {}, "sha256": "{}"}}]}}"#,
            quoted(evidence.to_str().unwrap()),
            evidence::digest(&py(&evidence)).unwrap()
        ),
    )
    .unwrap();
    assert!(refusal(fixture.run(true)).starts_with("workspace-dependent accounting: "));
    std::fs::write(
        attempt.join("budget-resolution.json"),
        r#"{"chargeUpperUsd": 0, "evidence": [{"path": "relative-to-another-controller-checkout.log"}]}"#,
    )
    .unwrap();
    assert!(
        refusal(fixture.run(true))
            .starts_with("relative accounting proof needs its original controller checkout: ")
    );
    assert!(evidence.exists());
}

#[test]
fn accounting_roots_must_be_named_and_present_and_readable() {
    let mut fixture = Fixture::new("roots");
    fixture.roots = Vec::new();
    fixture.save_plan();
    assert_eq!(
        refusal(fixture.run(true)),
        "inventory all controlling accounting queues explicitly"
    );
    let missing = fixture.base.join("missing-queue");
    fixture.roots = vec![missing.display().to_string()];
    fixture.save_plan();
    assert_eq!(
        refusal(fixture.run(true)),
        format!("accounting root missing: {}", missing.display())
    );
    fixture.roots = vec![fixture.queue.display().to_string()];
    fixture.save_plan();
    let hidden = fixture.queue.join("batches/run");
    std::fs::create_dir_all(&hidden).unwrap();
    std::fs::set_permissions(&hidden, std::os::unix::fs::PermissionsExt::from_mode(0o000)).unwrap();
    let message = failure(fixture.run(true));
    std::fs::set_permissions(&hidden, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    assert!(message.contains("Permission denied"), "{message}");
    assert!(fixture.work.join("stderr.log").exists());
}

#[test]
fn an_unsearchable_attempt_below_a_linked_ledger_is_an_error_not_no_reservation() {
    let mut fixture = Fixture::new("unsearchable-attempt");
    let queue = fixture.queue.clone();
    let attempt = fixture.attempt(&queue);
    symlink(&fixture.work, attempt.join("work")).unwrap();
    let moved = fixture.base.join("linked-batches");
    std::fs::rename(queue.join("batches"), &moved).unwrap();
    symlink(&moved, queue.join("batches")).unwrap();
    let hidden = moved.join("run/target/attempt-1");
    std::fs::set_permissions(&hidden, std::os::unix::fs::PermissionsExt::from_mode(0o000)).unwrap();
    let dependencies = fixture.dependencies();
    let message = failure(fixture.run(true));
    std::fs::set_permissions(&hidden, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    assert!(
        matches!(&dependencies, Err(Error::Io(found)) if found.contains("Permission denied")),
        "{dependencies:?}"
    );
    assert!(message.contains("Permission denied"), "{message}");
    assert!(fixture.work.join("dispatch.json").exists());
    assert!(!fixture.receipt().exists());
}

#[test]
fn an_unreadable_directory_in_the_workspace_is_an_error_not_an_empty_one() {
    let mut fixture = Fixture::new("unreadable");
    let hidden = fixture.work.join("hidden");
    std::fs::create_dir(&hidden).unwrap();
    std::fs::set_permissions(&hidden, std::os::unix::fs::PermissionsExt::from_mode(0o000)).unwrap();
    let message = failure(fixture.run(true));
    std::fs::set_permissions(&hidden, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    assert!(message.contains("Permission denied"), "{message}");
    assert!(fixture.work.join("stderr.log").exists() && hidden.exists());
}

#[test]
fn a_non_canonical_or_unsafe_workspace_refuses() {
    let mut fixture = Fixture::new("canonical");
    std::fs::create_dir(fixture.base.join("pivot")).unwrap();
    let alias = fixture.base.join("pivot/../work");
    fixture.workspace = alias.display().to_string();
    fixture.save_plan();
    assert!(refusal(fixture.run(true)).starts_with("workspace path must be canonical"));
    symlink(&fixture.base, fixture.base.join("link")).unwrap();
    fixture.workspace = fixture.base.join("link/work").display().to_string();
    fixture.save_plan();
    assert!(refusal(fixture.run(true)).starts_with("workspace path must be canonical"));
    for unsafe_root in [
        fixture.repo.clone(),
        fixture.base.clone(),
        std::fs::canonicalize("/tmp").unwrap(),
    ] {
        fixture.workspace = unsafe_root.display().to_string();
        fixture.save_plan();
        assert_eq!(refusal(fixture.run(true)), "unsafe workspace root");
    }
    assert!(fixture.work.join("stderr.log").exists());
}

#[test]
fn the_receipt_cannot_alias_the_workspace() {
    let mut fixture = Fixture::new("receipt");
    std::fs::create_dir(fixture.base.join("pivot")).unwrap();
    for receipt in [
        fixture.base.join("pivot/../work/retirement.json"),
        fixture.base.join("pivot/../work/../retirement.json"),
    ] {
        assert_eq!(
            refusal(fixture.run_with(true, &receipt, &idle, &removal::uninterrupted)),
            RECEIPT_REFUSAL
        );
    }
    assert!(fixture.work.join("stderr.log").exists());
    assert!(!fixture.work.join("retirement.json").exists());
}

#[test]
fn path_dependencies_do_not_confuse_unrelated_paths() {
    let fixture = Fixture::new("paths");
    let work = py(&fixture.work);
    assert!(!path_depends_on(&py(&fixture.base.join("work-other/file")), &work).unwrap());
    assert!(!path_depends_on(&py(&fixture.queue.join("dispatch.json")), &work).unwrap());
    assert!(path_depends_on(&py(&fixture.work.join("dispatch.json")), &work).unwrap());
}

#[test]
fn an_unknown_file_or_a_link_is_not_a_capture_shortcut() {
    let fixture = Fixture::new("capture");
    let manifest = evidence::validate(
        &evidence::read(&py(&fixture.repo.join(MANIFEST))).unwrap(),
        true,
    )
    .unwrap();
    let current = evidence_inventory::inventory(fixture.work.as_os_str().as_bytes(), true).unwrap();
    let class = at(&at(&current, "entries").as_array().unwrap()[0], "class").py_str();
    let unknown = json(
        &pyjson::dumps(&current, &pyjson::Dumps::default()).replacen(
            &format!(r#""class": "{class}""#),
            r#""class": "unknown""#,
            1,
        ),
    );
    let checked = capture_check(&unknown, &unknown, &manifest, PREFIX, false, &Value::Null);
    assert!(
        matches!(&checked, Err(Error::Violation(message)) if message.starts_with("unknown file blocks retirement")),
        "{checked:?}"
    );
    symlink(&fixture.repo, fixture.work.join("linked")).unwrap();
    let current = evidence_inventory::inventory(fixture.work.as_os_str().as_bytes(), true).unwrap();
    let checked = capture_check(&current, &current, &manifest, PREFIX, false, &Value::Null);
    assert!(
        matches!(&checked, Err(Error::Violation(message)) if message == "uncaptured non-file blocks retirement: linked"),
        "{checked:?}"
    );
}

/// Fails the `nth` time removal reaches `point`.
fn interrupt(
    point: Point,
    nth: u32,
    calls: &Cell<u32>,
) -> impl Fn(Point) -> std::io::Result<()> + '_ {
    move |reached| {
        if reached == point {
            calls.set(calls.get() + 1);
            if calls.get() == nth {
                return Err(std::io::Error::other("injected interruption"));
            }
        }
        Ok(())
    }
}

#[test]
fn an_interrupted_removal_reports_the_failure_and_a_retry_converges() {
    for (point, nth) in [
        (Point::BeforeRemoval, 1),
        (Point::Removed, 1),
        (Point::Removed, 2),
        (Point::BeforeReceipt, 1),
    ] {
        let mut fixture = Fixture::new(&format!("interrupt-{point:?}-{nth}"));
        let receipt = fixture.receipt();
        let calls = Cell::new(0);
        let hook = interrupt(point, nth, &calls);
        let message = failure(fixture.run_with(true, &receipt, &idle, &hook));
        assert_eq!(message, "injected interruption", "{point:?} {nth}");
        let started = evidence::read(&py(&receipt)).unwrap();
        assert_eq!(at(&started, "state").as_str(), Some(STARTED));
        assert!(!fixture.completed().exists());
        let left = ["dispatch.json", "stderr.log"]
            .iter()
            .filter(|name| fixture.work.join(name).exists())
            .count();
        match (point, nth) {
            (Point::BeforeRemoval, _) => assert_eq!(left, 2),
            (Point::Removed, 1) => assert_eq!(left, 1),
            _ => assert_eq!(left, 0),
        }
        assert!(
            refusal(fixture.run(false)).contains("rerun with --apply"),
            "{point:?}"
        );
        let finished = fixture.run(true).unwrap();
        assert!(matches!(at(&finished, "applied"), Value::Bool(true)));
        assert!(!fixture.work.exists(), "{point:?} {nth}");
        assert!(fixture.run(true).unwrap().py_eq(&finished));
    }
}

#[test]
fn a_file_added_or_changed_after_the_check_is_never_removed() {
    for change in ["added", "changed"] {
        let mut fixture = Fixture::new(change);
        let work = fixture.work.clone();
        let receipt = fixture.receipt();
        let hook = |point: Point| {
            if point == Point::BeforeRemoval {
                if change == "added" {
                    std::fs::write(work.join("late.log"), "late diagnostics")?;
                } else {
                    std::fs::write(work.join("stderr.log"), "rewritten diagnostics")?;
                }
            }
            Ok(())
        };
        let message = failure(fixture.run_with(true, &receipt, &idle, &hook));
        assert!(
            message.contains("never removed") || message.contains("changed since"),
            "{change}: {message}"
        );
        let kept = if change == "added" {
            "late.log"
        } else {
            "stderr.log"
        };
        assert!(work.join(kept).exists(), "{change}");
        assert!(!fixture.completed().exists());
        let again = failure(fixture.run(true));
        assert!(work.join(kept).exists(), "{change}: {again}");
    }
}

#[test]
fn a_substituted_workspace_is_never_removed() {
    for substitute in ["directory", "symlink"] {
        let mut fixture = Fixture::new(&format!("substitute-{substitute}"));
        let work = fixture.work.clone();
        let moved = fixture.base.join("moved");
        let receipt = fixture.receipt();
        let hook = |point: Point| {
            if point == Point::BeforeRemoval {
                std::fs::rename(&work, &moved)?;
                if substitute == "directory" {
                    std::fs::create_dir(&work)?;
                    std::fs::copy(moved.join("stderr.log"), work.join("stderr.log"))?;
                } else {
                    symlink(&moved, &work)?;
                }
            }
            Ok(())
        };
        let refused = refusal(fixture.run_with(true, &receipt, &idle, &hook));
        assert_eq!(refused, "workspace changed after retirement started");
        assert!(moved.join("stderr.log").exists() && moved.join("dispatch.json").exists());
        assert!(work.join("stderr.log").exists());
        assert!(!fixture.completed().exists());
    }
}

#[cfg(target_os = "linux")]
#[test]
fn active_users_finds_a_process_working_in_the_workspace() {
    let fixture = Fixture::new("active");
    let mut child = std::process::Command::new("sleep")
        .arg("30")
        .current_dir(&fixture.work)
        .spawn()
        .unwrap();
    let uid = std::fs::metadata("/proc/self").unwrap().uid();
    let process = PathBuf::from(format!("/proc/{}", child.id()));
    let found = uses(&process, &py(&fixture.work), uid);
    let elsewhere = uses(&process, &py(&fixture.queue), uid);
    let scanned = active_users(&py(&fixture.work));
    child.kill().unwrap();
    child.wait().unwrap();
    assert!(found.unwrap() && !elsewhere.unwrap());
    match scanned {
        Ok(pids) => assert!(pids.contains(&child.id()), "{pids:?}"),
        Err(Error::Violation(message)) => {
            assert!(message.starts_with("cannot inspect process "), "{message}")
        }
        Err(error) => panic!("{error:?}"),
    }
}

#[cfg(target_os = "macos")]
#[test]
fn active_users_refuses_without_linux_proc() {
    let fixture = Fixture::new("no-proc");
    match active_users(&py(&fixture.work)) {
        Err(Error::Violation(message)) => {
            assert_eq!(message, "cannot establish inactivity without Linux /proc");
        }
        other => panic!("{other:?}"),
    }
}

fn git(directory: &Path, arguments: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(directory)
        .args([
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
        ])
        .args(arguments)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

/// The checkout made a repository, with the workspace replaced by a linked worktree of it.
fn worktree(label: &str) -> Fixture {
    let mut fixture = Fixture::new(label);
    git(&fixture.repo, &["init", "-q"]);
    std::fs::write(fixture.repo.join(".gitignore"), "ignored/\n").unwrap();
    git(&fixture.repo, &["add", "."]);
    git(&fixture.repo, &["commit", "-qm", "fixture"]);
    std::fs::remove_dir_all(&fixture.work).unwrap();
    git(
        &fixture.repo,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "retire-fixture",
            fixture.work.to_str().unwrap(),
        ],
    );
    fixture.kind = "worktree";
    fixture.snapshot();
    fixture
}

#[test]
fn a_linked_worktree_needs_a_pushed_head_a_clean_tree_and_captured_ignored_files() {
    let mut fixture = worktree("worktree-refusals");
    assert!(matches!(
        at(&fixture.run(false).unwrap(), "applied"),
        Value::Bool(false)
    ));
    std::fs::remove_file(fixture.receipt()).unwrap();
    fixture.shared.unpushed = Some(git(&fixture.work, &["rev-parse", "HEAD"]));
    assert_eq!(refusal(fixture.run(true)), "worktree has unpushed commits");
    fixture.shared.unpushed = None;
    std::fs::create_dir(fixture.work.join("ignored")).unwrap();
    std::fs::write(
        fixture.work.join("ignored/diagnostics.log"),
        "uncaptured diagnostics",
    )
    .unwrap();
    fixture.snapshot();
    assert_eq!(
        refusal(fixture.run(true)),
        "uncaptured file blocks retirement: ignored/diagnostics.log"
    );
    std::fs::write(fixture.work.join(".gitignore"), "modified").unwrap();
    fixture.snapshot();
    assert_eq!(
        refusal(fixture.run(true)),
        "modified development work blocks retirement"
    );
    assert!(fixture.work.join(".gitignore").exists());
    fixture.kind = "review";
    std::fs::write(fixture.work.join(".gitignore"), "ignored/\n").unwrap();
    std::fs::remove_dir_all(fixture.work.join("ignored")).unwrap();
    fixture.snapshot();
    assert_eq!(
        refusal(fixture.run(true)),
        "Git worktrees require kind worktree"
    );
}

#[test]
fn a_retired_linked_worktree_leaves_its_branch_and_drops_only_its_registration() {
    let mut fixture = worktree("worktree-apply");
    let other = fixture.base.join("other");
    git(
        &fixture.repo,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "other",
            other.to_str().unwrap(),
        ],
    );
    let calls = Cell::new(0);
    let hook = interrupt(Point::Removed, 3, &calls);
    let receipt = fixture.receipt();
    failure(fixture.run_with(true, &receipt, &idle, &hook));
    let applied = fixture.run(true).unwrap();
    assert!(matches!(at(&applied, "applied"), Value::Bool(true)));
    assert!(!fixture.work.exists());
    let listed = git(&fixture.repo, &["worktree", "list", "--porcelain"]);
    assert!(
        !listed.contains(&format!("worktree {}\n", fixture.work.display())),
        "{listed}"
    );
    assert!(
        listed.contains(&format!("worktree {}", other.display())),
        "{listed}"
    );
    assert!(other.join(".gitignore").exists());
    git(&fixture.repo, &["rev-parse", "--verify", "retire-fixture"]);
}

#[test]
fn a_modified_nested_clone_refuses() {
    let mut fixture = Fixture::new("nested");
    let clone = fixture.work.join("clone");
    std::fs::create_dir(&clone).unwrap();
    git(&clone, &["init", "-q"]);
    std::fs::write(clone.join("draft.txt"), "uncommitted").unwrap();
    fixture.snapshot();
    assert_eq!(
        refusal(fixture.run(true)),
        format!("modified nested clone: {}", clone.display())
    );
    assert!(clone.join("draft.txt").exists());
}
