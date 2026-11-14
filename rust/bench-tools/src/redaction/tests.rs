use super::*;

const ADDRESS: &str = "person@example.com";
const ORGANIZATION_ID: &str = "11111111-2222-3333-4444-555555555555";

/// `"name":"value"`, assembled so that this source holds no field a rule would match.
fn pair(name: &str, value: &str) -> String {
    format!("\"{name}\":\"{value}\"")
}

/// `pair` inside a JSON string: each quote escaped once.
fn escaped(name: &str, value: &str) -> String {
    format!("\\\"{name}\\\":\\\"{value}\\\"")
}

fn sentence(address: &str) -> String {
    format!("The user's email {} {address}.", "address is")
}

fn values() -> Rules {
    Rules::from_values(
        br#"{"schema_version": 1, "email": ["person@example.com"],
            "organization": ["11111111-2222-3333-4444-555555555555"],
            "account": ["acct-0000-example"], "hosts": {"examplehost": "host-01-xyz"}}"#,
    )
    .unwrap()
}

fn text(data: &[u8], rules: &Rules) -> String {
    String::from_utf8(redact(data, rules).bytes(data).to_vec()).unwrap()
}

#[test]
fn field_rules_replace_each_value() {
    let rules = Rules::fields();
    assert_eq!(
        text(pair("organizationUuid", ORGANIZATION_ID).as_bytes(), &rules),
        pair("organizationUuid", "[redacted-organization]")
    );
    assert_eq!(
        text(sentence(ADDRESS).as_bytes(), &rules),
        sentence("[redacted-email]")
    );
    for name in ["creator_user_id", "creator_account_id"] {
        assert_eq!(
            text(pair(name, "user-abc123").as_bytes(), &rules),
            pair(name, "[redacted-account]")
        );
    }
    assert_eq!(
        text(pair("plan_type", "plus").as_bytes(), &rules),
        pair("plan_type", "[redacted-plan]")
    );
    let spaced = format!("{{\"plan_type\" : \"{}\"}}", "team");
    assert_eq!(
        text(spaced.as_bytes(), &rules),
        "{\"plan_type\" : \"[redacted-plan]\"}"
    );
}

#[test]
fn field_rules_reach_escaped_quotes() {
    let rules = Rules::fields();
    let once = escaped("organizationUuid", ORGANIZATION_ID);
    assert_eq!(
        text(once.as_bytes(), &rules),
        escaped("organizationUuid", "[redacted-organization]")
    );
    let twice = format!(
        "\\\\\\\"{}\\\\\\\":\\\\\\\"{}\\\\\\\"",
        "creator_user_id", "u-1"
    );
    assert_eq!(
        text(twice.as_bytes(), &rules),
        format!(
            "\\\\\\\"{}\\\\\\\":\\\\\\\"{}\\\\\\\"",
            "creator_user_id", "[redacted-account]"
        )
    );
    assert_eq!(
        text(escaped("plan_type", "pro").as_bytes(), &rules),
        escaped("plan_type", "[redacted-plan]")
    );
    let quoted = format!("The user\\u0027s email {} {ADDRESS}\\n", "address is");
    assert_eq!(
        text(quoted.as_bytes(), &rules),
        format!(
            "The user\\u0027s email {} [redacted-email]\\n",
            "address is"
        )
    );
}

#[test]
fn field_rules_leave_other_values_alone() {
    let rules = Rules::fields();
    for clean in [
        pair("organizationUuid", "not-an-id"),
        pair("plan_type", "x"),
        "\"creator_user_id\":null".to_string(),
        "The user's email address was given".to_string(),
        pair("organizationUuid", "[redacted-organization]"),
        pair("creator_account_id", "[redacted-account]"),
    ] {
        assert!(redact(clean.as_bytes(), &rules).clean(), "{clean}");
    }
}

#[test]
fn redaction_settles_after_one_pass() {
    let rules = values();
    let data = format!(
        "{} {} {} {} examplehost {ADDRESS} acct-0000-example",
        pair("organizationUuid", ORGANIZATION_ID),
        sentence(ADDRESS),
        escaped("creator_account_id", "acct-0000-example"),
        pair("plan_type", "plus"),
    );
    let once = redact(data.as_bytes(), &rules).data.unwrap();
    assert!(redact(&once, &rules).clean());
    let once = String::from_utf8(once).unwrap();
    assert!(
        !once.contains("examplehost") && !once.contains(ADDRESS) && !once.contains("acct-0000")
    );
    assert!(once.contains("host-01-xyz"));
}

#[test]
fn literals_replace_every_occurrence() {
    let rules = values();
    assert_eq!(
        text(b"to person@example.com and person@example.com", &rules),
        "to [redacted-email] and [redacted-email]"
    );
    assert_eq!(
        text(b"/srv/11111111-2222-3333-4444-555555555555/x", &rules),
        "/srv/[redacted-organization]/x"
    );
    assert_eq!(text(b"user@examplehost:~$", &rules), "user@host-01-xyz:~$");
}

/// A database of 512-byte pages: page 1 an empty table leaf, then one page of each kind with its cells at the end.
fn database(pages: &[(u8, &[u8])]) -> Vec<u8> {
    const SIZE: usize = 512;
    let mut data = vec![0; SIZE * (pages.len() + 1)];
    data[..16].copy_from_slice(SQLITE);
    data[16..18].copy_from_slice(&(SIZE as u16).to_be_bytes());
    data[100] = TABLE_LEAF;
    data[105..107].copy_from_slice(&(SIZE as u16).to_be_bytes());
    for (index, (kind, cells)) in pages.iter().enumerate() {
        let page = &mut data[SIZE * (index + 1)..SIZE * (index + 2)];
        let content = SIZE - cells.len();
        page[0] = *kind;
        page[5..7].copy_from_slice(&(content as u16).to_be_bytes());
        page[content..].copy_from_slice(cells);
    }
    data
}

const INDEX_LEAF: u8 = 0x0a;
const OVERFLOW: u8 = 0;

#[test]
fn sqlite_table_cells_are_rewritten_at_their_length() {
    let rules = values();
    let mut cells = pair("organizationUuid", ORGANIZATION_ID).into_bytes();
    cells.extend_from_slice(b" person@example.com examplehost ");
    cells.extend_from_slice(pair("plan_type", "plus").as_bytes());
    let data = database(&[(TABLE_LEAF, &cells), (INDEX_LEAF, b"no values here")]);
    assert!(redact(&database(&[(INDEX_LEAF, b"no values here")]), &rules).clean());
    let redacted = redact(&data, &rules);
    assert_eq!(
        redacted.findings,
        [Finding {
            member: vec![],
            unredactable: None
        }]
    );
    let clean = redacted.data.unwrap();
    assert_eq!(clean.len(), data.len());
    assert_eq!(clean[..512], data[..512]);
    assert_eq!(clean[1024..], data[1024..]);
    let shown = String::from_utf8_lossy(&clean);
    assert!(shown.contains(&pair(
        "organizationUuid",
        "[redacted-organization]-------------"
    )));
    assert!(shown.contains(" [redacted-email]-- host-01-xyz "));
    assert!(shown.contains(&pair("plan_type", "xxxx")));
    assert!(redact(&clean, &rules).clean());
}

#[test]
fn sqlite_values_outside_table_cells_are_reported_not_rewritten() {
    let rules = values();
    for kind in [INDEX_LEAF, OVERFLOW] {
        let data = database(&[
            (TABLE_LEAF, b" person@example.com "),
            (kind, b" person@example.com "),
        ]);
        let redacted = redact(&data, &rules);
        assert!(redacted.data.is_none());
        assert_eq!(
            redacted.findings,
            [Finding {
                member: vec![],
                unredactable: Some("a value lies outside the cells of a SQLite table leaf page")
            }]
        );
    }
    let mut header = database(&[(TABLE_LEAF, b"plain")]);
    header[530..548].copy_from_slice(ADDRESS.as_bytes());
    assert!(redact(&header, &rules).data.is_none());
}

#[test]
fn sqlite_write_ahead_logs_are_reported_not_rewritten() {
    let rules = values();
    let mut wal = b"\x37\x7f\x06\x82".to_vec();
    wal.extend_from_slice(b" plain frames");
    assert!(redact(&wal, &rules).clean());
    wal.extend_from_slice(b" acct-0000-example");
    let redacted = redact(&wal, &rules);
    assert!(redacted.data.is_none());
    assert_eq!(
        redacted.findings,
        [Finding {
            member: vec![],
            unredactable: Some("SQLite write-ahead logs are not rewritten")
        }]
    );
}

fn gzip(data: &[u8]) -> Vec<u8> {
    let mut writer = archive::gzip_writer(Vec::new(), 9);
    writer.write_all(data).unwrap();
    writer.finish().unwrap()
}

fn gunzip(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    archive::GzipReader::new(io::BufReader::new(data))
        .read_to_end(&mut out)
        .unwrap();
    out
}

fn tarball(members: &[(&str, u32, &[u8])]) -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    let mut directory = tar::Header::new_ustar();
    directory.set_entry_type(tar::EntryType::Directory);
    directory.set_path("sessions/").unwrap();
    directory.set_size(0);
    directory.set_mode(0o755);
    directory.set_cksum();
    builder.append(&directory, io::empty()).unwrap();
    for (name, mode, data) in members {
        let mut header = tar::Header::new_ustar();
        header.set_path(name).unwrap();
        header.set_size(data.len() as u64);
        header.set_mode(*mode);
        header.set_cksum();
        builder.append(&header, *data).unwrap();
    }
    builder.into_inner().unwrap()
}

fn listing(data: &[u8]) -> Vec<(String, u32, Vec<u8>)> {
    let mut bundle = tar::Archive::new(data);
    bundle
        .entries()
        .unwrap()
        .map(|entry| {
            let mut entry = entry.unwrap();
            let mut body = Vec::new();
            entry.read_to_end(&mut body).unwrap();
            let name = entry.path().unwrap().display().to_string();
            (name, entry.header().mode().unwrap(), body)
        })
        .collect()
}

#[test]
fn gzip_round_trips_and_is_left_alone_when_clean() {
    let rules = Rules::fields();
    let clean = gzip(b"nothing to see");
    assert!(redact(&clean, &rules).clean());
    let line = pair("plan_type", "plus");
    let dirty = gzip(line.as_bytes());
    let redacted = redact(&dirty, &rules);
    assert_eq!(
        redacted.findings,
        [Finding {
            member: vec![],
            unredactable: None
        }]
    );
    assert_eq!(
        gunzip(&redacted.data.unwrap()),
        pair("plan_type", "[redacted-plan]").as_bytes()
    );
}

#[test]
fn tar_members_change_and_keep_names_modes_and_order() {
    let rules = Rules::fields();
    let session = pair("organizationUuid", ORGANIZATION_ID);
    let original = gzip(&tarball(&[
        ("sessions/a.jsonl", 0o600, session.as_bytes()),
        ("sessions/b.txt", 0o644, b"plain"),
    ]));
    assert!(redact(&gzip(&tarball(&[("x", 0o644, b"plain")])), &rules).clean());
    let redacted = redact(&original, &rules);
    assert_eq!(
        redacted.findings,
        [Finding {
            member: vec![b"sessions/a.jsonl".to_vec()],
            unredactable: None
        }]
    );
    assert_eq!(
        redacted.findings[0].describe("x.tar.gz"),
        "would change: x.tar.gz!sessions/a.jsonl"
    );
    let rows = listing(&gunzip(&redacted.data.unwrap()));
    let names: Vec<(&str, u32)> = rows
        .iter()
        .map(|(name, mode, _)| (name.as_str(), *mode))
        .collect();
    assert_eq!(
        names,
        [
            ("sessions/", 0o755),
            ("sessions/a.jsonl", 0o600),
            ("sessions/b.txt", 0o644)
        ]
    );
    assert_eq!(
        rows[1].2,
        pair("organizationUuid", "[redacted-organization]").as_bytes()
    );
    assert_eq!(rows[2].2, b"plain");
}

#[test]
fn zip_members_are_reported_not_rewritten() {
    let rules = Rules::fields();
    let mut bundle = zip::ZipWriter::new(io::Cursor::new(Vec::new()));
    bundle
        .start_file(
            "inner/session.jsonl",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
    bundle
        .write_all(pair("plan_type", "plus").as_bytes())
        .unwrap();
    let data = bundle.finish().unwrap().into_inner();
    let redacted = redact(&data, &rules);
    assert!(redacted.data.is_none());
    assert_eq!(
        redacted.findings,
        [Finding {
            member: vec![b"inner/session.jsonl".to_vec()],
            unredactable: Some("zip members are not rewritten")
        }]
    );
    assert!(would_change(&data, &rules));
}

#[test]
fn xz_streams_are_opaque() {
    let mut data = XZ.to_vec();
    data.extend_from_slice(pair("plan_type", "plus").as_bytes());
    assert!(redact(&data, &Rules::fields()).clean());
}

#[test]
fn values_files_are_checked() {
    let refused = |text: &str| Rules::from_values(text.as_bytes()).unwrap_err().0;
    assert_eq!(refused(r#"{"email": []}"#), "schema_version must be 1");
    assert_eq!(
        refused(r#"{"schema_version": 1, "email": ["a@b"]}"#),
        "an email value is shorter than 4 bytes"
    );
    assert_eq!(
        refused(r#"{"schema_version": 1, "hosts": {"examplehost": "host-01"}}"#),
        "the stand-in 'host-01' differs in length from the machine name it replaces"
    );
    assert_eq!(
        refused(r#"{"schema_version": 1, "plan": ["plus"]}"#),
        "unknown key 'plan'"
    );
    assert_eq!(
        refused(r#"{"schema_version": 1, "account": ["redacted"]}"#),
        "a value occurs inside a placeholder or stand-in, so redaction would not settle"
    );
    assert!(refused("not json").starts_with("not JSON"));
    assert!(
        !refused(r#"{"schema_version": 1, "email": ["secret-value@example.com", 3]}"#)
            .contains("secret")
    );
}

#[test]
fn a_missing_values_file_leaves_the_field_rules() {
    let base = crate::test_scratch("redaction-tests").join(crate::evidence::random_name("v-"));
    std::fs::create_dir_all(&base).unwrap();
    let rules = Rules::from_path(&base.join("absent.json")).unwrap();
    assert!(rules.values_file().is_none());
    assert_eq!(
        text(pair("plan_type", "plus").as_bytes(), &rules),
        pair("plan_type", "[redacted-plan]")
    );
    let file = base.join("values.json");
    std::fs::write(
        &file,
        r#"{"schema_version": 1, "email": ["person@example.com"]}"#,
    )
    .unwrap();
    let rules = Rules::from_path(&file).unwrap();
    assert_eq!(rules.values_file(), Some(file.as_path()));
    assert_eq!(text(b"person@example.com", &rules), "[redacted-email]");
    std::fs::write(&file, "{").unwrap();
    assert!(Rules::from_path(&file).unwrap_err().0.contains("not JSON"));
}

#[test]
fn apply_rewrites_only_what_changes() {
    let base = crate::test_scratch("redaction-tests").join(crate::evidence::random_name("a-"));
    std::fs::create_dir_all(base.join("nested")).unwrap();
    let dirty = base.join("nested/session.jsonl");
    let clean = base.join("clean.txt");
    std::fs::write(&dirty, pair("plan_type", "plus")).unwrap();
    std::fs::set_permissions(&dirty, std::fs::Permissions::from_mode(0o640)).unwrap();
    std::fs::write(&clean, "nothing").unwrap();
    let before = std::fs::metadata(&clean).unwrap().modified().unwrap();
    let applied = apply(std::slice::from_ref(&base), &Rules::fields()).unwrap();
    assert_eq!(applied.rewritten, [shown(&dirty)]);
    assert_eq!(applied.unredactable, Vec::<String>::new());
    assert_eq!(
        std::fs::read_to_string(&dirty).unwrap(),
        pair("plan_type", "[redacted-plan]")
    );
    assert_eq!(
        std::fs::metadata(&dirty).unwrap().permissions().mode() & 0o777,
        0o640
    );
    assert_eq!(
        std::fs::metadata(&clean).unwrap().modified().unwrap(),
        before
    );
    let sources = Sources {
        paths: vec![base],
        ..Sources::default()
    };
    assert_eq!(
        check(&sources, &Rules::fields()).unwrap(),
        Vec::<String>::new()
    );
}
