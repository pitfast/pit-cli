use std::fs;
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
        .current_dir(&fixture)
        .args(["run", "--env", "MODE=production", "--", "arg1"])
        .output()
        .expect("pit run should start");
    assert!(
        run.status.success(),
        "pit run failed:\n{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(String::from_utf8_lossy(&run.stdout).contains("Hello from PitCrew P2!"));
    assert!(String::from_utf8_lossy(&run.stdout).contains("arg=arg1"));
    assert!(String::from_utf8_lossy(&run.stdout).contains("MODE=production"));

    let cached = Command::new(env!("CARGO_BIN_EXE_pit"))
        .current_dir(&fixture)
        .arg("build")
        .output()
        .expect("cached pit build should start");
    assert!(cached.status.success());
    assert!(String::from_utf8_lossy(&cached.stdout).contains("Reusing cached artifact"));

    let source_path = fixture.join("src/main.rs");
    let source = fs::read_to_string(&source_path).unwrap();
    let known_good = fs::read(fixture.join(".pit/build/rust-hello.wasm")).unwrap();
    fs::write(&source_path, "fn main( {").unwrap();
    let failed_build = Command::new(env!("CARGO_BIN_EXE_pit"))
        .current_dir(&fixture)
        .arg("build")
        .output()
        .expect("failed build should start");
    assert!(!failed_build.status.success());
    assert_eq!(
        fs::read(fixture.join(".pit/build/rust-hello.wasm")).unwrap(),
        known_good
    );
    fs::write(&source_path, source).unwrap();

    let p1 = Command::new(env!("CARGO_BIN_EXE_pit"))
        .current_dir(&fixture)
        .args(["build", "--abi", "wasi-preview1"])
        .output()
        .expect("P1 pit build should start");
    assert!(p1.status.success());
    let p1_run = Command::new(env!("CARGO_BIN_EXE_pit"))
        .current_dir(&fixture)
        .arg("run")
        .output()
        .expect("P1 pit run should start");
    assert!(p1_run.status.success());
    assert!(String::from_utf8_lossy(&p1_run.stdout).contains("Hello from PitCrew!"));

    // Previous-milestone P1 manifests did not carry the format field.
    let manifest_path = fixture.join(".pit/artifact.json");
    let mut legacy: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    legacy["runtime"].as_object_mut().unwrap().remove("format");
    fs::write(&manifest_path, serde_json::to_vec_pretty(&legacy).unwrap()).unwrap();
    let legacy_run = Command::new(env!("CARGO_BIN_EXE_pit"))
        .current_dir(&fixture)
        .arg("run")
        .output()
        .expect("legacy P1 pit run should start");
    assert!(legacy_run.status.success());

    let restore_p2 = Command::new(env!("CARGO_BIN_EXE_pit"))
        .current_dir(&fixture)
        .arg("build")
        .output()
        .expect("P2 restore build should start");
    assert!(restore_p2.status.success());
    let artifact_path = fixture.join(".pit/build/rust-hello.wasm");
    let mut corrupted = fs::read(&artifact_path).unwrap();
    corrupted.push(0);
    fs::write(&artifact_path, corrupted).unwrap();
    let corrupt_inspect = Command::new(env!("CARGO_BIN_EXE_pit"))
        .current_dir(&fixture)
        .arg("inspect")
        .output()
        .expect("corrupt inspect should start");
    assert!(!corrupt_inspect.status.success());
    let corrupt_run = Command::new(env!("CARGO_BIN_EXE_pit"))
        .current_dir(&fixture)
        .arg("run")
        .output()
        .expect("corrupt run should start");
    assert!(!corrupt_run.status.success());
    let recovered = Command::new(env!("CARGO_BIN_EXE_pit"))
        .current_dir(&fixture)
        .arg("build")
        .output()
        .expect("recovery build should start");
    assert!(recovered.status.success());

    let inspect = Command::new(env!("CARGO_BIN_EXE_pit"))
        .current_dir(&fixture)
        .arg("inspect")
        .output()
        .expect("pit inspect should start");
    assert!(inspect.status.success());
    assert!(String::from_utf8_lossy(&inspect.stdout).contains("runtime ABI supported"));

    let clean = Command::new(env!("CARGO_BIN_EXE_pit"))
        .current_dir(&fixture)
        .arg("clean")
        .output()
        .expect("pit clean should start");
    assert!(clean.status.success());
    assert!(!fixture.join(".pit").exists());
    assert!(fixture.join("Cargo.toml").exists());
}
