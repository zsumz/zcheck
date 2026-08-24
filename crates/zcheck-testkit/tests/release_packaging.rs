//! Checked-in release helpers create deterministic archives and reviewed notes.

#![cfg(unix)]

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn release_archives_are_deterministic_with_exact_members() {
    let root = repository_root();
    let temporary = temporary_directory("archives");
    assert!(fs::create_dir_all(&temporary).is_ok());
    let license = temporary.join("LICENSE");
    let readme = temporary.join("README.md");
    assert!(fs::write(&license, "license\n").is_ok());
    assert!(fs::write(&readme, "readme\n").is_ok());

    for (suffix, executable) in [("tar.gz", "zcheck"), ("zip", "zcheck.exe")] {
        let binary = temporary.join(executable);
        let output = temporary.join(format!("zcheck-1.2.3-target.{suffix}"));
        assert!(fs::write(&binary, b"binary\0bytes").is_ok());
        assert!(package(&root, &binary, &license, &readme, &output));
        let first = fs::read(&output);
        assert!(first.is_ok());

        assert!(fs::write(&binary, b"binary\0bytes").is_ok());
        assert!(fs::write(&license, "license\n").is_ok());
        assert!(package(&root, &binary, &license, &readme, &output));
        let second = fs::read(&output);
        assert!(second.is_ok());

        assert_eq!(
            first.as_deref().ok(),
            second.as_deref().ok(),
            "{suffix} output depends on filesystem metadata"
        );
        assert!(archive_members(&output).as_ref().is_some_and(|members| {
            members == &[executable.to_owned(), "LICENSE".into(), "README.md".into()]
        }));
    }
    assert!(fs::remove_dir_all(temporary).is_ok());
}

#[test]
fn release_notes_are_extracted_from_one_exact_version_section() {
    let root = repository_root();
    let temporary = temporary_directory("notes");
    assert!(fs::create_dir_all(&temporary).is_ok());
    let changelog = temporary.join("CHANGELOG.md");
    let output = temporary.join("notes.md");
    assert!(
        fs::write(
            &changelog,
            "# Changelog\n\n## 1.2.3 - today\n\nReviewed notes.\n\n## 1.2.2\n\nOld.\n",
        )
        .is_ok()
    );

    let status = Command::new("python3")
        .arg(root.join("scripts/release-notes.py"))
        .args(["1.2.3"])
        .arg(&changelog)
        .arg(&output)
        .status()
        .ok();
    assert!(
        status
            .as_ref()
            .is_some_and(std::process::ExitStatus::success)
    );
    assert!(
        fs::read_to_string(&output)
            .as_ref()
            .is_ok_and(|notes| notes == "Reviewed notes.\n")
    );

    let missing = Command::new("python3")
        .arg(root.join("scripts/release-notes.py"))
        .args(["9.9.9"])
        .arg(&changelog)
        .arg(&output)
        .output()
        .ok();
    assert!(
        missing
            .as_ref()
            .is_some_and(|value| !value.status.success())
    );
    assert!(fs::remove_dir_all(temporary).is_ok());
}

fn package(root: &Path, binary: &Path, license: &Path, readme: &Path, output: &Path) -> bool {
    Command::new("python3")
        .arg(root.join("scripts/package-release.py"))
        .arg("--binary")
        .arg(binary)
        .arg("--license")
        .arg(license)
        .arg("--readme")
        .arg(readme)
        .arg("--output")
        .arg(output)
        .status()
        .is_ok_and(|status| status.success())
}

fn archive_members(archive: &Path) -> Option<Vec<String>> {
    let script = concat!(
        "import sys,tarfile,zipfile; p=sys.argv[1]; ",
        "names=tarfile.open(p,'r:gz').getnames() if p.endswith('.tar.gz') ",
        "else zipfile.ZipFile(p).namelist(); print('\\n'.join(names))"
    );
    let output = Command::new("python3")
        .args(["-c", script])
        .arg(archive)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(
        String::from_utf8(output.stdout)
            .ok()?
            .lines()
            .map(str::to_owned)
            .collect(),
    )
}

fn temporary_directory(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    std::env::temp_dir().join(format!(
        "zcheck-release-{label}-{}-{nonce}",
        std::process::id()
    ))
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .map_or_else(PathBuf::new, Path::to_path_buf)
}
