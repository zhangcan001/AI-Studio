use std::process::Command;

use ai_studio_lib::application::build_info::BUILD_COMMIT;

#[test]
fn embedded_build_commit_matches_current_git_head() {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("git must be available in a checked-out source tree");
    assert!(output.status.success(), "git rev-parse HEAD must succeed");
    let expected = String::from_utf8(output.stdout)
        .expect("git HEAD must be valid UTF-8")
        .trim()
        .to_owned();

    assert_eq!(BUILD_COMMIT, expected);
}
