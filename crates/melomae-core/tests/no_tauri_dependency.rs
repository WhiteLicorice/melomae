use std::collections::HashSet;

use cargo_metadata::MetadataCommand;

#[test]
fn melomae_core_has_no_tauri_dependency() {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let metadata = MetadataCommand::new()
        .manifest_path(manifest)
        .exec()
        .expect("cargo metadata failed");

    let core = metadata
        .packages
        .iter()
        .find(|package| package.name == "melomae-core")
        .expect("melomae-core is not in the workspace");
    let resolve = metadata
        .resolve
        .as_ref()
        .expect("cargo metadata returned no resolve graph");

    let mut seen = HashSet::new();
    let mut pending = vec![core.id.clone()];
    let mut offenders = Vec::new();

    while let Some(id) = pending.pop() {
        if !seen.insert(id.clone()) {
            continue;
        }
        let Some(node) = resolve.nodes.iter().find(|node| node.id == id) else {
            continue;
        };
        for dependency in &node.dependencies {
            let package = metadata
                .packages
                .iter()
                .find(|package| package.id == *dependency)
                .expect("a dependency package is missing from the metadata");
            if package.name.starts_with("tauri") {
                offenders.push(package.name.clone());
            }
            pending.push(dependency.clone());
        }
    }

    assert!(
        offenders.is_empty(),
        "melomae-core must not depend on any tauri crate, found: {offenders:?}"
    );
}
