use std::path::PathBuf;

use runtime::cedar_authz::CedarAuthz;

fn nonexistent_policy_dir() -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("clawdia-missing-policies-{}", std::process::id()));
    p
}

#[tokio::test]
async fn cedarling_load_fails_on_missing_policy_dir() {
    let dir = nonexistent_policy_dir();
    let result = CedarAuthz::from_directory(&dir).await;
    assert!(
        result.is_err(),
        "expected Cedar load to fail for missing dir {}",
        dir.display(),
    );
}
