use std::process::Command;

#[test]
fn build_then_run_fixture_without_explicit_wasm_path() {
    let fixture =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../pit-crew/fixtures/rust-hello");
    let build = Command::new(env!("CARGO_BIN_EXE_pit"))
        .current_dir(&fixture)
        .arg("build")
        .output()
        .expect("pit build should start");
    assert!(
        build.status.success(),
        "pit build failed:\n{}",
        String::from_utf8_lossy(&build.stderr)
    );
    assert!(fixture.join(".pit/artifact.json").is_file());
    assert!(fixture.join(".pit/build/rust-hello.wasm").is_file());

    let run = Command::new(env!("CARGO_BIN_EXE_pit"))
        .current_dir(fixture)
        .args(["run"])
        .output()
        .expect("pit run should start");
    assert!(
        run.status.success(),
        "pit run failed:\n{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(String::from_utf8_lossy(&run.stdout).contains("Hello from PitCrew!"));
}
