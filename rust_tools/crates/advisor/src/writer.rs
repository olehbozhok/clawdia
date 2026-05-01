//! Writes generated artifacts to a target directory atomically.
//!
//! Strategy: write everything to a tempdir, then `std::fs::rename` to the
//! target path. If the target already exists, it is removed first (caller is
//! expected to track the previous version via git).

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::types::Artifacts;

/// Write all artifacts to `target`. Overwrites any existing directory.
pub fn write_artifacts(target: &Path, artifacts: &Artifacts) -> Result<()> {
    let staging = tempfile::TempDir::new().context("creating staging tempdir")?;
    let staging_path = staging.path();

    fs::create_dir_all(staging_path.join("policies"))?;
    fs::create_dir_all(staging_path.join("entities"))?;

    fs::write(staging_path.join("schema.cedarschema"), &artifacts.schema)?;
    fs::write(staging_path.join("metadata.json"), &artifacts.metadata_json)?;
    fs::write(
        staging_path.join("entities/agents.json"),
        &artifacts.agents_json,
    )?;
    fs::write(
        staging_path.join("entities/system.json"),
        &artifacts.system_json,
    )?;

    for (agent, src) in &artifacts.policies {
        let path = staging_path.join("policies").join(format!("{agent}.cedar"));
        fs::write(&path, src).with_context(|| format!("writing {}", path.display()))?;
    }

    if target.exists() {
        fs::remove_dir_all(target)
            .with_context(|| format!("removing existing target {}", target.display()))?;
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }

    // Persist the tempdir contents into the target. `rename` works only if
    // both paths are on the same filesystem; copy as fallback.
    let staging_persist = staging.keep();
    if fs::rename(&staging_persist, target).is_err() {
        copy_dir_all(&staging_persist, target)?;
        fs::remove_dir_all(&staging_persist).ok();
    }

    Ok(())
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dst_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dst_path)?;
        } else {
            fs::copy(entry.path(), dst_path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn fixture() -> Artifacts {
        let mut policies = BTreeMap::new();
        policies.insert("researcher".to_string(), "// pol".to_string());
        Artifacts {
            schema: "namespace AgentPolicy { entity Agent = {agent_type: String}; entity System; }"
                .to_string(),
            policies,
            agents_json: "[]".to_string(),
            system_json: "[]".to_string(),
            metadata_json: "{}".to_string(),
        }
    }

    #[test]
    fn writes_all_files() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("out");
        write_artifacts(&target, &fixture()).unwrap();

        assert!(target.join("schema.cedarschema").exists());
        assert!(target.join("metadata.json").exists());
        assert!(target.join("entities/agents.json").exists());
        assert!(target.join("entities/system.json").exists());
        assert!(target.join("policies/researcher.cedar").exists());
    }

    #[test]
    fn overwrites_existing_target() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("out");
        write_artifacts(&target, &fixture()).unwrap();
        // Add a stray file that should be wiped.
        fs::write(target.join("stray.txt"), "x").unwrap();
        write_artifacts(&target, &fixture()).unwrap();
        assert!(!target.join("stray.txt").exists());
    }
}
