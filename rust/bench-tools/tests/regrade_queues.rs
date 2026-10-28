use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

#[test]
fn blocked_queues_continue_and_other_stops_take_precedence() {
    for (codes, expected, launched) in [
        ([0, 0, 0], 0, 3),
        ([5, 0, 0], 5, 3),
        ([0, 5, 5], 5, 3),
        ([5, 1, 0], 1, 2),
        ([5, 2, 0], 2, 2),
        ([5, 3, 0], 3, 2),
        ([5, 4, 0], 4, 2),
    ] {
        let root = std::env::temp_dir().join(format!(
            "regrade-queues-{}-{}-{}",
            std::process::id(),
            codes[0],
            codes[1]
        ));
        fs::create_dir_all(root.join("bench/tools")).unwrap();
        let driver = root.join("bench/tools/regrade_queues.sh");
        fs::write(
            &driver,
            include_str!("../../../bench/tools/regrade_queues.sh"),
        )
        .unwrap();
        let tools = root.join("bench/tools/bench-tools");
        fs::write(
            &tools,
            r#"#!/bin/sh
while [ "$#" -gt 0 ]; do
  case "$1" in
    --authorization) authorization=$2; shift 2 ;;
    --directory) queue=$2; shift 2 ;;
    *) shift ;;
  esac
done
printf '%s\n' "$queue" >> "$REGRADE_DRIVER_CALLS"
exit "$(cat "$authorization")"
"#,
        )
        .unwrap();
        fs::set_permissions(&tools, fs::Permissions::from_mode(0o755)).unwrap();
        let calls = root.join("calls");
        let mut command = Command::new("bash");
        command
            .arg(&driver)
            .args(["run", "logs", "1"])
            .env("TMPDIR", root.join("tmp"))
            .env("REGRADE_DRIVER_CALLS", &calls)
            .env_remove("LIMIT");
        for (index, code) in codes.iter().enumerate() {
            let authorization = format!("authorization-{index}");
            fs::write(root.join(&authorization), code.to_string()).unwrap();
            command.arg(format!("{authorization}=queue-{index}"));
        }
        let output = command.output().unwrap();
        assert_eq!(
            output.status.code(),
            Some(expected),
            "codes={codes:?}, stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
        let actual = fs::read_to_string(calls).unwrap();
        let queues: Vec<_> = (0..launched)
            .map(|index| format!("queue-{index}"))
            .collect();
        assert_eq!(actual.lines().collect::<Vec<_>>(), queues);
        let log = fs::read_to_string(root.join("logs/driver.log")).unwrap();
        for (index, code) in codes.iter().take(launched).enumerate() {
            assert!(log.contains(&format!("end queue-{index} exit={code}")));
            if *code == 5 {
                assert!(log.contains(&format!("queue-{index} finished with blocked batches")));
            }
        }
        fs::remove_dir_all(root).unwrap();
    }
}
