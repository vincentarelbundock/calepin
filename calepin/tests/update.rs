use std::path::Path;
use std::process::Command;

fn calepin_bin() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_calepin"))
}

#[cfg(unix)]
fn write_updater_script(dir: &Path, executable: bool) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let updater = dir.join("calepin-update");
    std::fs::write(
        &updater,
        "#!/bin/sh\nprintf 'fake updater invoked\\n'\nexit 23\n",
    )
    .unwrap();
    let mut permissions = std::fs::metadata(&updater).unwrap().permissions();
    permissions.set_mode(if executable { 0o755 } else { 0o644 });
    std::fs::set_permissions(&updater, permissions).unwrap();
    updater
}

#[cfg(unix)]
fn write_fake_updater(dir: &Path) -> std::path::PathBuf {
    write_updater_script(dir, true)
}

#[cfg(unix)]
#[test]
fn update_delegates_to_calepin_update_on_path() {
    let dir = tempfile::tempdir().unwrap();
    let _updater = write_fake_updater(dir.path());
    let path = std::env::join_paths([dir.path()]).unwrap();

    let output = Command::new(calepin_bin())
        .arg("update")
        .env("PATH", path)
        .output()
        .expect("failed to run calepin update");

    assert_eq!(output.status.code(), Some(23));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "fake updater invoked\n"
    );
}

#[cfg(unix)]
#[test]
fn update_skips_non_executable_calepin_update_on_path() {
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let _non_executable = write_updater_script(first.path(), false);
    let _updater = write_fake_updater(second.path());
    let path = std::env::join_paths([first.path(), second.path()]).unwrap();

    let output = Command::new(calepin_bin())
        .arg("update")
        .env("PATH", path)
        .output()
        .expect("failed to run calepin update");

    assert_eq!(output.status.code(), Some(23));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "fake updater invoked\n"
    );
}
