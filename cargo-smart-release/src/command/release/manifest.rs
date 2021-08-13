use super::{
    cargo, git,
    utils::{names_and_versions, package_by_id, package_eq_dependency},
    Context, Options,
};
use cargo_metadata::{Metadata, Package};
use git_repository::hash::ObjectId;
use std::{collections::BTreeMap, str::FromStr};

pub(in crate::command::release_impl) fn edit_version_and_fixup_dependent_crates(
    meta: &Metadata,
    publishees: &[(&Package, String)],
    empty_commit_possible: bool,
    Options {
        dry_run, allow_dirty, ..
    }: Options,
    ctx: &Context,
) -> anyhow::Result<ObjectId> {
    if !allow_dirty {
        git::assure_clean_working_tree()?;
    }
    let mut locks_by_manifest_path = BTreeMap::new();
    for (publishee, _) in publishees {
        let lock = git_lock::File::acquire_to_update_resource(
            &publishee.manifest_path,
            git_lock::acquire::Fail::Immediately,
            None,
        )?;
        locks_by_manifest_path.insert(&publishee.manifest_path, lock);
    }
    let mut packages_to_fix = Vec::new();
    for package_to_fix in meta
        .workspace_members
        .iter()
        .map(|id| package_by_id(meta, id))
        .filter(|p| {
            p.dependencies.iter().any(|dep| {
                publishees
                    .iter()
                    .any(|(publishee, _)| package_eq_dependency(publishee, dep))
            })
        })
    {
        if locks_by_manifest_path.contains_key(&package_to_fix.manifest_path) {
            continue;
        }
        let lock = git_lock::File::acquire_to_update_resource(
            &package_to_fix.manifest_path,
            git_lock::acquire::Fail::Immediately,
            None,
        )?;
        locks_by_manifest_path.insert(&package_to_fix.manifest_path, lock);
        packages_to_fix.push(package_to_fix);
    }

    for (publishee, new_version) in publishees {
        let mut lock = locks_by_manifest_path
            .get_mut(&publishee.manifest_path)
            .expect("lock available");
        set_version_and_update_package_dependency(publishee, Some(&new_version.to_string()), publishees, &mut lock)?;
    }

    for package_to_update in packages_to_fix.iter_mut() {
        let mut lock = locks_by_manifest_path
            .get_mut(&package_to_update.manifest_path)
            .expect("lock written once");
        set_version_and_update_package_dependency(package_to_update, None, publishees, &mut lock)?;
    }

    let message = format!("Release {}", names_and_versions(publishees));
    if dry_run {
        log::info!("WOULD commit changes to manifests with {:?}", message);
        Ok(ObjectId::null_sha1())
    } else {
        log::info!("Persisting changes to manifests");
        for manifest_lock in locks_by_manifest_path.into_values() {
            manifest_lock.commit()?;
        }
        // This is dangerous as incompatibilities can happen here, leaving the working tree dirty.
        // For now we leave it that way without auto-restoring originals to facilitate debugging.
        cargo::refresh_lock_file()?;
        git::commit_changes(message, empty_commit_possible, ctx)
    }
}

fn set_version_and_update_package_dependency(
    package_to_update: &Package,
    new_version: Option<&str>,
    publishees: &[(&Package, String)],
    mut out: impl std::io::Write,
) -> anyhow::Result<()> {
    let manifest = std::fs::read_to_string(&package_to_update.manifest_path)?;
    let mut doc = toml_edit::Document::from_str(&manifest)?;

    if let Some(new_version) = new_version {
        doc["package"]["version"] = toml_edit::value(new_version);
        log::info!(
            "Pending '{}' manifest version update: \"{}\"",
            package_to_update.name,
            new_version
        );
    }
    for dep_type in &["dependencies", "dev-dependencies", "build-dependencies"] {
        for (name_to_find, new_version) in publishees.iter().map(|(p, nv)| (&p.name, nv)) {
            for name_to_find in package_to_update
                .dependencies
                .iter()
                .filter(|dep| &dep.name == name_to_find)
                .map(|dep| dep.rename.as_ref().unwrap_or_else(|| &dep.name))
            {
                if let Some(current_version) = doc
                    .as_table_mut()
                    .get_mut(dep_type)
                    .and_then(|deps| deps.as_table_mut())
                    .and_then(|deps| deps.get_mut(name_to_find).and_then(|name| name.as_inline_table_mut()))
                    .and_then(|name_table| name_table.get_mut("version"))
                {
                    log::info!(
                        "Pending '{}' manifest {} update: '{} = \"{}\"' (from {})",
                        package_to_update.name,
                        dep_type,
                        name_to_find,
                        new_version,
                        current_version
                    );
                    *current_version = toml_edit::Value::from(new_version.as_str());
                }
            }
        }
    }
    out.write_all(doc.to_string_in_original_order().as_bytes())?;

    Ok(())
}