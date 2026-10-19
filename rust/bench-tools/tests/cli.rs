use std::process::{Command, Output};

fn bench_tools(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_bench-tools"))
        .args(arguments)
        .output()
        .unwrap()
}

/// No corpus reaches the top level, whose programs all start with a subcommand.
#[test]
fn top_level_exit_codes() {
    for (arguments, code) in [
        (&[][..], 2),
        (&["-h"][..], 0),
        (&["--help"][..], 0),
        (&["-hx"][..], 2),
        (&["-hroster"][..], 2),
        (&["--he"][..], 2),
        (&["help"][..], 2),
        (&["ros"][..], 2),
        (&["unknown"][..], 2),
    ] {
        let output = bench_tools(arguments);
        assert_eq!(output.status.code(), Some(code), "{arguments:?}");
        let text = if code == 0 {
            &output.stdout
        } else {
            &output.stderr
        };
        assert!(
            String::from_utf8_lossy(text).contains("Usage: bench-tools <COMMAND>"),
            "{arguments:?}"
        );
    }
}

/// argparse refuses `-h` with `=` or `-` attached in every subcommand, as the replaced parser did.
#[test]
fn help_with_equals_or_dash_is_refused() {
    for command in [
        "roster",
        "diff-identity",
        "normalize-review",
        "normalize-thermo",
        "derive-packet",
        "build-packet",
        "check-manifest",
        "rates",
        "upstream",
        "export-explorer",
    ] {
        for (help, explicit) in [
            ("-h=1", "'1'"),
            ("-h=", "''"),
            ("-h-", "'-'"),
            ("-h-x", "'-x'"),
        ] {
            let output = bench_tools(&[command, help]);
            assert_eq!(output.status.code(), Some(2), "{command} {help}");
            assert!(
                String::from_utf8_lossy(&output.stderr).starts_with(&format!(
                    "error: argument -h/--help: ignored explicit argument {explicit}\n"
                )),
                "{command} {help}"
            );
        }
    }
}
