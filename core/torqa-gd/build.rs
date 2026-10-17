//! Link settings for the GDExtension library, and the commit it is built from.

use std::path::Path;
use std::process::Command;

fn main() {
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    // rav1d's x86 assembly addresses rav1d's tables directly, as if they could not be replaced,
    // but a shared library's exported symbols can be on Linux, and the linker refuses that.
    // Binding the library's references to its own definitions makes the assembly's assumption
    // true (ADR 0010).
    if os == "linux" && arch == "x86_64" {
        println!("cargo:rustc-cdylib-link-arg=-Wl,-Bsymbolic");
    }
    commit();
    println!("cargo:rerun-if-changed=build.rs");
}

/// The commit the About tab shows (R83, ADR 0018), as `TORQA_COMMIT`: that variable's value
/// when set, as `scripts/dev.sh` sets it for worktrees, whose Git directory the container
/// cannot see; else Git's; empty without either, e.g. built from a source archive.
fn commit() {
    println!("cargo:rerun-if-env-changed=TORQA_COMMIT");
    let commit = std::env::var("TORQA_COMMIT")
        .ok()
        .filter(|commit| !commit.is_empty())
        .or_else(git_commit)
        .unwrap_or_default();
    println!("cargo:rustc-env=TORQA_COMMIT={commit}");
}

fn git_commit() -> Option<String> {
    let commit = git(&["rev-parse", "--short=7", "HEAD"])?;
    // A commit or a checkout rewrites HEAD or adds to its log: build again for the new one.
    for name in ["HEAD", "logs/HEAD"] {
        if let Some(path) = git(&["rev-parse", "--path-format=absolute", "--git-path", name])
            && Path::new(&path).exists()
        {
            println!("cargo:rerun-if-changed={path}");
        }
    }
    Some(commit)
}

/// Git's answer, trimmed; `None` if Git is missing or fails.
fn git(args: &[&str]) -> Option<String> {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").ok()?;
    let output = Command::new("git")
        // The checkout can belong to another user than the build, as in CI's container, and
        // Git refuses such a repository unless told it is safe. This only reads it.
        .args(["-c", "safe.directory=*", "-C", &manifest])
        .args(args)
        .output()
        .ok()?;
    let answer = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    (output.status.success() && !answer.is_empty()).then_some(answer)
}
