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

#[test]
fn literals_replace_escaped_encoded_and_recased_forms() {
    let rules = values();
    for written in [
        r#"(?i)person@example\.com"#,
        r#""person@example\\.com""#,
        r#"person\@example\.com"#,
        "person%40example.com",
        "person%40example%2Ecom",
        "Person@Example.COM",
    ] {
        let redacted = text(format!("<{written}>").as_bytes(), &rules);
        assert!(
            redacted.contains("[redacted-email]") && !redacted.contains("example"),
            "{written} became {redacted}"
        );
    }
    assert_eq!(text(b"acct\\-0000\\-example", &rules), "[redacted-account]");
    assert_eq!(text(b"EXAMPLEHOST", &rules), "host-01-xyz");
    for unrelated in [
        "personXexample.com",
        "person@example-com",
        "acct-0000_example",
    ] {
        assert!(redact(unrelated.as_bytes(), &rules).clean(), "{unrelated}");
    }
}

#[test]
fn sqlite_files_are_reported_not_rewritten() {
    let rules = values();
    for magic in [SQLITE, WAL[0], WAL[1]] {
        let mut data = magic.to_vec();
        data.extend_from_slice(b" plain pages ");
        assert!(redact(&data, &rules).clean());
        data.extend_from_slice(ADDRESS.as_bytes());
        let redacted = redact(&data, &rules);
        assert!(redacted.data.is_none());
        assert_eq!(
            redacted.findings,
            [Finding {
                member: vec![],
                unredactable: Some("SQLite files are not rewritten")
            }]
        );
    }
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
        check(&sources, &Rules::fields()).unwrap().lines,
        Vec::<String>::new()
    );
}

fn finding(unredactable: Option<&'static str>) -> Finding {
    Finding {
        member: vec![],
        unredactable,
    }
}

#[test]
fn the_remedy_fits_each_kind_of_finding() {
    let changes = remedy(&[finding(None)]);
    assert!(changes.contains("redaction apply") && !changes.contains("leave it out"));
    let beyond = remedy(&[finding(Some("SQLite files are not rewritten"))]);
    assert!(!beyond.contains("redaction apply") && beyond.contains("leave it out"));
    let both = remedy(&[
        finding(None),
        finding(Some("zip members are not rewritten")),
    ]);
    assert!(both.contains("redaction apply") && both.contains("leave it out"));
}

#[test]
fn check_keeps_the_findings_behind_its_lines() {
    let base = crate::test_scratch("redaction-tests").join(crate::evidence::random_name("k-"));
    std::fs::create_dir_all(&base).unwrap();
    let mut database = SQLITE.to_vec();
    database.extend_from_slice(ADDRESS.as_bytes());
    std::fs::write(base.join("a.db"), database).unwrap();
    std::fs::write(base.join("b.txt"), "mail person@example.com").unwrap();
    let sources = Sources {
        paths: vec![base.clone()],
        ..Sources::default()
    };
    let checked = check(&sources, &values()).unwrap();
    assert_eq!(
        checked.lines,
        [
            format!(
                "cannot redact: {} (SQLite files are not rewritten)",
                shown(&base.join("a.db"))
            ),
            format!("would change: {}", shown(&base.join("b.txt"))),
        ]
    );
    let advice = remedy(&checked.findings);
    assert!(advice.contains("redaction apply") && advice.contains("leave it out"));
    assert!(!advice.contains("person@"));
}

/// The start of the owner's personal instructions inside a JSON string, assembled so that this source holds no match.
fn instructions(rest: &str) -> String {
    let nl = "\\n";
    format!(
        "# AGENTS.md{nl}{nl}{} {rest}",
        "Make the smallest complete change that satisfies the request."
    )
}

#[test]
fn personal_instructions_end_at_the_string_or_the_reminder() {
    let rules = Rules::fields();
    let nl = "\\n";
    let attachment = format!(
        "{{\"content\":\"{}\",\"type\":\"Project\"}}",
        instructions(&format!("Quote \\\"x\\\"{nl}more text."))
    );
    assert_eq!(
        text(attachment.as_bytes(), &rules),
        "{\"content\":\"[redacted-instructions]\",\"type\":\"Project\"}"
    );
    let reminder = format!(
        "{{\"text\":\"Contents of f (private):{nl}{nl}{}{nl}</system-reminder>\"}}",
        instructions("Body.")
    );
    assert_eq!(
        text(reminder.as_bytes(), &rules),
        format!(
            "{{\"text\":\"Contents of f (private):{nl}{nl}[redacted-instructions]{nl}</system-reminder>\"}}"
        )
    );
    let codex = format!(
        "{{\"text\":\"<INSTRUCTIONS>{nl}{}{nl}{nl}--- project-doc ---{nl}{nl}# Other\"}}",
        instructions("Body.")
    );
    assert_eq!(
        text(codex.as_bytes(), &rules),
        format!(
            "{{\"text\":\"<INSTRUCTIONS>{nl}[redacted-instructions]{nl}{nl}--- project-doc ---{nl}{nl}# Other\"}}"
        )
    );
    let plain = "# AGENTS.md\n\nSomething else";
    assert!(redact(plain.as_bytes(), &rules).bytes(plain.as_bytes()) == plain.as_bytes());
}

#[test]
fn windows_account_and_skill_names_are_replaced_once() {
    let rules = Rules::fields();
    let path = format!(
        "/mnt/c/{}/someone/iCloudDrive/m/.agents/skills/my-skill/SKILL.md",
        "Users"
    );
    let replaced = format!(
        "/mnt/c/{}/[redacted-user]/iCloudDrive/m/.agents/skills/[redacted-skill]/SKILL.md",
        "Users"
    );
    assert_eq!(text(path.as_bytes(), &rules), replaced);
    let again = redact(replaced.as_bytes(), &rules);
    assert!(again.clean());
    let other = format!("path = \"/mnt/d/{}/someone/.claude/x\"", "Users");
    assert_eq!(
        text(other.as_bytes(), &rules),
        format!("path = \"/mnt/d/{}/[redacted-user]/.claude/x\"", "Users")
    );
}

#[test]
fn service_user_skill_folders_are_replaced_except_builtin_ones() {
    let rules = Rules::fields();
    let line =
        |dir: &str, name: &str| format!("path = \"/home/t3/.t3/agents/{dir}/{name}/SKILL.md\"");
    assert_eq!(
        text(line(".agents_skills", "my-skill").as_bytes(), &rules),
        line(".agents_skills", "[redacted-skill]")
    );
    let builtin = line(".codex/skills", ".system");
    assert!(redact(builtin.as_bytes(), &rules).clean());
}

#[test]
fn the_installed_skill_list_is_replaced_once() {
    let rules = Rules::fields();
    let nl = "\\n";
    let list = format!("{nl}### Available {}{nl}", "skills");
    let message =
        format!("{{\"text\":\"head{list}- a: b (file: c){nl}- d: e{nl}</skills_instructions>\"}}");
    let replaced =
        format!("{{\"text\":\"head{list}[redacted-skill-list]{nl}</skills_instructions>\"}}");
    assert_eq!(text(message.as_bytes(), &rules), replaced);
    assert!(redact(replaced.as_bytes(), &rules).clean());
}

/// What [`replaced_values`] finds in what `rules` make of `data`.
fn confirmed(data: &[u8], rules: &Rules) -> Result<Vec<Replaced>> {
    let redacted = redact(data, rules);
    replaced_values(data, redacted.bytes(data), &rules.stand_ins())
}

fn counted(pairs: &[(&[u8], usize)]) -> Vec<Replaced> {
    pairs
        .iter()
        .map(|(text, count)| (text.to_vec(), *count))
        .collect()
}

#[test]
fn the_check_without_values_accepts_what_each_rule_writes() {
    let nl = "\\n";
    let rules = values();
    let fields = format!(
        "{} {} {} {}",
        pair("organizationUuid", ORGANIZATION_ID),
        sentence("someone@example.org"),
        escaped("creator_account_id", "user-abc123"),
        pair("plan_type", "plus"),
    );
    assert_eq!(
        confirmed(fields.as_bytes(), &Rules::fields()).unwrap(),
        counted(&[(ACCOUNT, 1), (EMAIL, 1), (ORGANIZATION, 1), (PLAN, 1)])
    );
    let literals =
        "ssh examplehost; mail person@example.com, then person@example.com; examplehost.local\n";
    assert_eq!(
        confirmed(literals.as_bytes(), &rules).unwrap(),
        counted(&[(EMAIL, 2), (b"host-01-xyz", 2)])
    );
    let attachment = format!(
        "{{\"content\":\"{}\",\"type\":\"Project\"}}",
        instructions(&format!("Quote \\\"x\\\"{nl}more text."))
    );
    let list = format!("{nl}### Available {}{nl}", "skills");
    let skills = format!("{{\"text\":\"head{list}- a: b{nl}- d: e{nl}</skills_instructions>\"}}");
    let paths = format!(
        "/mnt/c/{}/someone/iCloudDrive/m/.agents/skills/my-skill/SKILL.md and /home/t3/.t3/agents/{}/x/y",
        "Users", ".agents_skills"
    );
    assert_eq!(
        confirmed(attachment.as_bytes(), &rules).unwrap(),
        counted(&[(INSTRUCTIONS, 1)])
    );
    assert_eq!(
        confirmed(skills.as_bytes(), &rules).unwrap(),
        counted(&[(SKILL_LIST, 1)])
    );
    assert_eq!(
        confirmed(paths.as_bytes(), &rules).unwrap(),
        counted(&[(SKILL, 2), (USER, 1)])
    );
    let again = "already [redacted-email] and host-01-xyz, now examplehost";
    assert_eq!(
        confirmed(again.as_bytes(), &rules).unwrap(),
        counted(&[(b"host-01-xyz", 1)])
    );
}

#[test]
fn the_check_without_values_refuses_anything_else() {
    let refused = |before: &str, after: &str, stand_ins: &[&[u8]]| {
        replaced_values(before.as_bytes(), after.as_bytes(), stand_ins)
            .unwrap_err()
            .0
    };
    let parted = |at: usize| {
        format!(
            "the redacted bytes are not the earlier bytes with values replaced by placeholders; they part at byte \
             {at} of the redacted bytes"
        )
    };
    let stand_in: &[&[u8]] = &[b"host-01-xyz"];
    assert_eq!(
        refused("mail a@b.example now", "mail [redacted-email] then", &[]),
        parted(5)
    );
    assert_eq!(
        refused(
            "one\ntwo three\nfour\n",
            "one\n[redacted-account]\nfour\n",
            &[]
        ),
        parted(4)
    );
    let long = format!("x {} y", "a".repeat(300));
    assert_eq!(refused(&long, "x [redacted-account] y", &[]), parted(2));
    assert_eq!(refused("abc", "abcd", &[]), parted(0));
    assert_eq!(
        refused("ssh shorthost now", "ssh host-01-xyz now", stand_in),
        parted(4)
    );
    assert_eq!(
        refused(
            "ssh examplehost or examplehost",
            "ssh host-01-xyz or examplehost",
            stand_in
        ),
        parted(4)
    );
    assert_eq!(
        refused(
            "ssh examplehost or otherhost00",
            "ssh host-01-xyz or host-01-xyz",
            stand_in
        ),
        "the stand-in 'host-01-xyz' replaced two different values"
    );
    assert_eq!(
        refused("a b", "a b", &[b"ab"]),
        "'ab' cannot be the stand-in of a machine name"
    );
    assert_eq!(
        replaced_values(b"same", b"same", &[]).unwrap(),
        Vec::<Replaced>::new()
    );
}

#[test]
fn a_gap_holds_only_what_its_rule_can_replace() {
    let parted = |at: usize| {
        format!(
            "the redacted bytes are not the earlier bytes with values replaced by placeholders; they part at byte \
             {at} of the redacted bytes"
        )
    };
    let refused = |before: &str, after: &str, stand_ins: &[&[u8]]| {
        replaced_values(before.as_bytes(), after.as_bytes(), stand_ins)
            .unwrap_err()
            .0
    };
    assert_eq!(
        refused(
            "to a@b.example,cc everyone",
            "to [redacted-email] everyone",
            &[]
        ),
        parted(3)
    );
    assert_eq!(
        refused("id acct-1,acct-2 here", "id [redacted-account] here", &[]),
        parted(3)
    );
    assert_eq!(
        refused("on box(1) now", "on host(1) now", &[b"host(1)"]),
        "'host(1)' cannot be the stand-in of a machine name"
    );
    assert_eq!(
        refused(
            "on box(1)xyzab now",
            "on host-01-xyz now",
            &[b"host-01-xyz"]
        ),
        parted(3)
    );
    let users = format!("/srv/{}/someone/x", "Users");
    assert_eq!(
        refused(&users, &users.replace("someone", "[redacted-user]"), &[]),
        parted(11)
    );
    let mounted = format!("/mnt/c/{}/some.one", "Users");
    assert_eq!(
        refused(&mounted, &mounted.replace("some", "[redacted-user]"), &[]),
        parted(13)
    );
    let skills = format!("/x/.agents/{}/my-skill.extra/", "skills");
    assert_eq!(
        refused(
            &skills,
            &skills.replace("my-skill", "[redacted-skill]"),
            &[]
        ),
        parted(18)
    );
    assert_eq!(
        replaced_values(
            b"to a@b.example everyone",
            b"to [redacted-email] everyone",
            &[]
        )
        .unwrap(),
        counted(&[(EMAIL, 1)])
    );
}

#[test]
fn an_alignment_with_too_many_ways_is_refused() {
    let before = "a".repeat(5000);
    let after = "[redacted-account]a".repeat(40);
    assert_eq!(
        replaced_values(before.as_bytes(), after.as_bytes(), &[])
            .unwrap_err()
            .0,
        "the redacted bytes align with the earlier bytes in too many ways to re-check, from byte 304 of the redacted \
         bytes"
    );
}
