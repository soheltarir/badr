use serde_json::Value;
use std::{fs, process::Command};

// This same public CLI check runs against the extracted release executable.
#[test]
fn installed_binary_manages_records_with_only_trusted_git_on_path() {
    for human in [false, true] {
        workflow(human);
    }
}

fn workflow(human: bool) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("repository");
    fs::create_dir(&root).unwrap();
    let tools = dir.path().join("bin");
    fs::create_dir(&tools).unwrap();
    let git = Command::new("/bin/sh")
        .args(["-c", "command -v git"])
        .output()
        .unwrap();
    assert!(git.status.success());
    std::os::unix::fs::symlink(
        String::from_utf8(git.stdout).unwrap().trim(),
        tools.join("git"),
    )
    .unwrap();
    let binary = std::env::var_os("BADR_TEST_BINARY")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_badr").into());
    let binary = fs::canonicalize(binary).unwrap();
    let git = |args: &[&str]| {
        let out = Command::new(tools.join("git"))
            .env_clear()
            .env("HOME", dir.path())
            .env("PATH", &tools)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .args(["-C"])
            .arg(&root)
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    };
    git(&["init", "-b", "trunk"]);
    git(&[
        "-c",
        "user.name=Test",
        "-c",
        "user.email=test@example.invalid",
        "commit",
        "--allow-empty",
        "-m",
        "baseline",
    ]);
    git(&["switch", "-c", "feature"]);
    let run = |args: &[&str], json: bool, exit: i32| {
        let mut command = Command::new(&binary);
        command
            .env_clear()
            .env("HOME", dir.path())
            .env("PATH", &tools)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .arg("--repo")
            .arg(&root);
        if json {
            command.arg("--json");
        }
        let out = command.args(args).output().unwrap();
        assert_eq!(
            out.status.code(),
            Some(exit),
            "{args:?}: {} {}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        if json {
            assert!(
                out.stderr.is_empty(),
                "{args:?}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
        out.stdout
    };
    let check = |args: &[&str]| -> Value { serde_json::from_slice(&run(args, true, 0)).unwrap() };
    let authorized = |args: &[&str]| {
        let mut args = args.to_vec();
        args.extend([
            "--confirm",
            "--actor",
            "Human",
            "--rationale",
            "Release acceptance",
        ]);
        let out = run(&args, !human, 0);
        assert!(!out.is_empty());
    };
    run(&["init", "--baseline", "trunk"], !human, 0);
    authorized(&["domain", "add", "ordering", "--prefix", "ORD"]);
    for title in ["first", "second"] {
        run(&["new", "adr", title], !human, 0);
        run(
            &[
                "new", "brr", title, "--domain", "ordering", "--owner", "Product",
            ],
            !human,
            0,
        );
    }
    let brr_path = |number, title| {
        root.join(format!(
            ".badr/brrs/domains/ordering/rules/BRR-ORD-{number:03}-{title}.feature"
        ))
    };
    for (number, title) in [(1, "first"), (2, "second")] {
        let path = brr_path(number, title);
        let text = fs::read_to_string(&path)
            .unwrap()
            .replace("TODO describe", "Describe")
            .replace("Feature:", "@successor\nFeature:");
        fs::write(path, text + "    @compatibility:existing-data @treatment:grandfather\n    Scenario: Existing data\n      Then existing rights remain\n    @compatibility:in-flight @treatment:re-evaluate\n    Scenario: In flight\n      Then eligibility is re-evaluated\n").unwrap();
        authorized(&["propose", &format!("BRR-ORD-{number:03}")]);
        let path = root.join(format!(".badr/adrs/{number:04}-{title}.md"));
        let text = fs::read_to_string(&path)
            .unwrap()
            .replace("TODO:", "Reviewed:");
        fs::write(path, text).unwrap();
    }
    for id in ["0001", "0002", "BRR-ORD-001", "BRR-ORD-002"] {
        authorized(&["decide", id, "--reviewed"]);
    }
    for (id, path) in [
        ("0001", root.join(".badr/adrs/0001-first.md")),
        ("BRR-ORD-001", brr_path(1, "first")),
    ] {
        let input = dir.path().join("revision");
        fs::write(
            &input,
            fs::read_to_string(path)
                .unwrap()
                .replace("first", "Revised first"),
        )
        .unwrap();
        authorized(&["edit", id, "--content", input.to_str().unwrap()]);
        assert_eq!(check(&["show", id])["records"][0]["status"], "proposed");
        authorized(&["decide", id, "--reviewed"]);
    }
    authorized(&["link", "BRR-ORD-001", "0001"]);
    check(&["related", "BRR-ORD-001"]);
    authorized(&["unlink", "BRR-ORD-001", "0001"]);
    authorized(&["activate", "BRR-ORD-001"]);
    authorized(&["activate", "BRR-ORD-002"]);
    for (old, new) in [("0001", "0002"), ("BRR-ORD-001", "BRR-ORD-002")] {
        let mut args = vec![
            "supersede",
            "--predecessor",
            old,
            "--successor",
            new,
            "--complete",
        ];
        if old.starts_with("BRR-") {
            args.push("--reviewed");
        }
        authorized(&args);
        assert_eq!(check(&["show", old])["records"][0]["status"], "superseded");
    }
    authorized(&["retire", "BRR-ORD-002"]);
    authorized(&[
        "deprecate",
        "0002",
        "--decision-gap",
        "None",
        "--existing-code-treatment",
        "Retain existing code",
    ]);
    let result = check(&["validate"]);
    assert_eq!(result["schema_version"], 1);
    assert_eq!(result["pending_commit_durability"], true);
    assert_eq!(run(&["list"], true, 0), run(&["list"], true, 0));
    let human = run(&["list"], false, 0);
    assert!(!human.is_empty());
    run(&["show", "missing"], true, 2);
    fs::write(root.join(".badr.lock"), "pid=99999999\n").unwrap();
    run(&["new", "adr", "locked"], true, 4);
    fs::remove_file(root.join(".badr.lock")).unwrap();
    git(&["branch", "-D", "trunk"]);
    run(&["validate"], true, 3);
    assert_eq!(fs::read_dir(&tools).unwrap().count(), 1);
}
