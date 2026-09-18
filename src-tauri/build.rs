use std::process::Command;

fn git_output(args: &[&str]) -> Option<String> {
    let manifest_dir = std::env::var_os("CARGO_MANIFEST_DIR")?;
    let output = Command::new("git")
        .args(args)
        .current_dir(manifest_dir)
        .output()
        .ok()
        .filter(|output| output.status.success())?;
    String::from_utf8(output.stdout)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn main() {
    println!("cargo:rerun-if-env-changed=AI_STUDIO_BUILD_COMMIT");

    for path in ["HEAD", "refs", "packed-refs"] {
        let args = ["rev-parse", "--path-format=absolute", "--git-path", path];
        if let Some(path) = git_output(&args) {
            println!("cargo:rerun-if-changed={path}");
        }
    }

    let commit = std::env::var("AI_STUDIO_BUILD_COMMIT")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| git_output(&["rev-parse", "HEAD"]))
        .unwrap_or_else(|| "unknown".to_owned());
    println!("cargo:rustc-env=AI_STUDIO_BUILD_COMMIT={commit}");
    tauri_build::build()
}
