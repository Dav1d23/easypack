use assert_cmd::prelude::*; // Add methods on commands
use predicates::prelude::*; // Used for writing assertions

use std::{process::Command, str::FromStr}; // Run programs

#[test]
fn unknown_command() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("easypack")?;

    cmd.arg("dunno");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Unkown command: dunno"));

    Ok(())
}

#[test]
fn only_command() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("easypack")?;

    cmd.arg("pack");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Not enough arguments."));

    Ok(())
}

#[test]
fn file_doesnt_exist() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("easypack")?;

    cmd.arg("pack")
        .arg("test/file/doesnt/exist")
        .arg("name")
        .arg("file");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("No such file or directory"));

    Ok(())
}

#[test]
fn wrong_number_argument_pack() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("easypack")?;

    cmd.arg("pack")
        .arg("outfile")
        .arg("name")
        .arg("file")
        .arg("shouldnotbehere");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Arguments must be"));

    Ok(())
}

#[test]
fn wrong_number_argument_unpack() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("easypack")?;

    cmd.arg("unpack")
        .arg("outfile")
        .arg("name")
        .arg("file")
        .arg("shouldnotbehere");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Arguments must be"));

    Ok(())
}

#[test]
fn test_complete_pack_unpack() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("easypack")?;

    let binpath = std::path::PathBuf::from_str("afile.bin")?;
    let oldmain = std::path::PathBuf::from_str("src/main.rs")?;
    let oldlib = std::path::PathBuf::from_str("src/lib.rs")?;
    let newmain = std::path::PathBuf::from_str("newmainfile.rs")?;
    let newlib = std::path::PathBuf::from_str("newlibfile.rs")?;

    cmd.arg("pack")
        .arg(binpath.as_path())
        .arg("main")
        .arg(oldmain.as_path())
        .arg("lib")
        .arg(oldlib.as_path());
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("easypack")?;
    cmd.arg("unpack")
        .arg(binpath.as_path())
        .arg("main")
        .arg(newmain.as_path())
        .arg("lib")
        .arg(newlib.as_path());
    cmd.assert().success();

    // Verify 2 files are created.
    let predicate_fn = predicate::path::is_file();
    assert!(predicate_fn.eval(&newmain));
    assert!(predicate_fn.eval(&newlib));

    // Verify that the main is copied exactly.
    let predicate_file = predicate::path::eq_file(oldmain.as_path());
    assert!(predicate_file.eval(newmain.as_path()));
    assert!(!predicate_file.eval(newlib.as_path()));
    // ... and the same for lib.
    let predicate_file = predicate::path::eq_file(oldlib.as_path());
    assert!(!predicate_file.eval(newmain.as_path()));
    assert!(predicate_file.eval(newlib.as_path()));

    // Cleanup.
    std::fs::remove_file(binpath)
        .unwrap_or_else(|e| eprintln!("Unable to remove `afile.bin`: {}", e));
    std::fs::remove_file(newmain)
        .unwrap_or_else(|e| eprintln!("Unable to remove `newmainfile.rs`: {}", e));
    std::fs::remove_file(newlib)
        .unwrap_or_else(|e| eprintln!("Unable to remove `newlibfile.rs`: {}", e));

    Ok(())
}
