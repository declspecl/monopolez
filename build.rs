use std::process::Command;

fn git(arguments: &[&str]) -> Option<String> {
    let output = Command::new("git").args(arguments).output().ok()?;
    output.status.success().then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn main() {
    for path in ["src", "Cargo.toml", "Cargo.lock", "build.rs"] {
        println!("cargo:rerun-if-changed={path}");
    }
    for name in ["HEAD", "index", "packed-refs"] {
        if let Some(path) = git(&["rev-parse", "--git-path", name]) {
            println!("cargo:rerun-if-changed={path}");
        }
    }
    if let Some(reference) = git(&["symbolic-ref", "-q", "HEAD"])
        && let Some(path) = git(&["rev-parse", "--git-path", &reference])
    {
        println!("cargo:rerun-if-changed={path}");
    }

    let revision = git(&["rev-parse", "--verify", "HEAD"]).unwrap_or_else(|| "unknown".into());
    let dirty = git(&["status", "--porcelain", "--untracked-files=all", "--", "src", "Cargo.toml", "Cargo.lock", "build.rs"])
        .map(|status| (!status.is_empty()).to_string())
        .unwrap_or_else(|| "unknown".into());
    let compiler = Command::new(std::env::var_os("RUSTC").expect("Cargo supplies RUSTC"))
        .arg("--version")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=MONOPOLEZ_REVISION={revision}");
    println!("cargo:rustc-env=MONOPOLEZ_DIRTY={dirty}");
    println!("cargo:rustc-env=MONOPOLEZ_COMPILER={compiler}");
    for variable in ["TARGET", "PROFILE", "OPT_LEVEL"] {
        println!("cargo:rustc-env=MONOPOLEZ_{variable}={}", std::env::var(variable).expect("Cargo supplies build settings"));
    }
}
