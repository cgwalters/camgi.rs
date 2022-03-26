// Copyright (C) 2022 Red Hat
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::prelude::*;
use crate::resources::Node;
use std::fs;
use std::path::{Path, PathBuf};

pub struct MustGather {
    pub title: String,
    pub version: String,
    pub nodes: Vec<Node>,
}

impl MustGather {
    /// Build a MustGather from a path to a directory containing the root.
    pub fn from(path: String) -> Result<MustGather> {
        let path = find_must_gather_root(path)?;
        let title = String::from(path.file_name().unwrap().to_str().unwrap());
        let version = get_cluster_version(&path);
        let nodes = get_nodes(&path);

        Ok(MustGather {
            title,
            version,
            nodes,
        })
    }
}

/// Build a path to a resource, does not guarantee that it exists.
/// If a name is provided the path will include a yaml file. If the name is
/// an empty string the path will be to the directory containing the resource
/// manifest yaml files.
/// If the namespace is an emptry string then the path will be to cluster
/// scoped resources.
/// Example - finding node resources
/// build_manifest_path(mgroot, "", "", "nodes", "core")
/// Example - finding a specific machine
/// build_manifest_path(mgroot, "machine-name", "openshift-machine-api", "machines", "machine.openshift.io")
fn build_manifest_path(
    path: &Path,
    name: &str,
    namespace: &str,
    kind: &str,
    group: &str,
) -> PathBuf {
    let mut manifestpath = path.to_path_buf();

    if namespace.is_empty() {
        manifestpath.push("cluster-scoped-resources");
    } else {
        manifestpath.push("namespaces");
        manifestpath.push(namespace);
    }

    if !group.is_empty() {
        manifestpath.push(group);
    }

    manifestpath.push(kind);

    if !name.is_empty() {
        manifestpath.push(format!("{}.yaml", name));
    }

    manifestpath
}

/// Find the root of a must-gather directory structure given a path.
///
/// Finding the root of the must-gather is accomplished through the following steps:
/// 1. look for a `version` file in the current path, if it exists return current path.
/// 2. look for the directories `namespaces` and `cluster-scoped-resources` in the current path,
///    if they exist, return the current path.
/// 3. if there is a single subdirectory in the path, recursively run this function on it and
///    return the result.
/// 4. return an error
fn find_must_gather_root(path: String) -> Result<PathBuf> {
    let orig = PathBuf::from(&path);
    let vpath: PathBuf = [String::from(&path), String::from("version")]
        .iter()
        .collect();
    let npath: PathBuf = [String::from(&path), String::from("namespaces")]
        .iter()
        .collect();
    let csrpath: PathBuf = [
        String::from(&path),
        String::from("cluster-scoped-resources"),
    ]
    .iter()
    .collect();

    if vpath.is_file() || (npath.is_dir() && csrpath.is_dir()) {
        return Ok(orig.canonicalize().unwrap());
    }

    let directories: Vec<PathBuf> = fs::read_dir(orig)
        .unwrap()
        .into_iter()
        .filter(|r| r.is_ok())
        .map(|r| r.unwrap().path())
        .filter(|r| r.is_dir())
        .collect();

    if directories.len() == 1 {
        find_must_gather_root(String::from(directories[0].to_str().unwrap()))
    } else {
        Err(anyhow::anyhow!("Cannot determine root of must-gather"))
    }
}

/// Get the version string.
/// If unable to determine the version, "Unknown" will be returned.
fn get_cluster_version(path: &Path) -> String {
    let mut manifestpath =
        build_manifest_path(path, "", "", "clusterversions", "config.openshift.io");
    manifestpath.push("version.yaml");
    let version = match Manifest::from(manifestpath) {
        Ok(v) => v,
        Err(_) => return String::from("Unknown"),
    };
    match version.as_yaml()["status"]["desired"]["version"].as_str() {
        Some(v) => String::from(v),
        None => String::from("Unknown"),
    }
}

/// Get all the Nodes in the cluster.
fn get_nodes(path: &Path) -> Vec<Node> {
    let mut nodes = Vec::new();
    let manifestpath = build_manifest_path(path, "", "", "nodes", "core");
    let yamlfiles: Vec<PathBuf> = fs::read_dir(&manifestpath)
        .unwrap()
        .into_iter()
        .filter(|m| m.is_ok())
        .map(|m| m.unwrap().path())
        .filter(|m| m.extension().unwrap() == "yaml")
        .collect();

    for path in yamlfiles {
        match Manifest::from(path) {
            Ok(m) => nodes.push(Node::from(m)),
            Err(_) => continue,
        }
    }
    nodes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_manifest_path_cluster_scoped() {
        assert_eq!(
            build_manifest_path(&PathBuf::from("/foo"), "", "", "nodes", "core"),
            PathBuf::from("/foo/cluster-scoped-resources/core/nodes")
        )
    }

    #[test]
    fn test_build_manifest_path_cluster_scoped_named_resource() {
        assert_eq!(
            build_manifest_path(&PathBuf::from("/foo"), "node1", "", "nodes", "core"),
            PathBuf::from("/foo/cluster-scoped-resources/core/nodes/node1.yaml")
        )
    }

    #[test]
    fn test_build_manifest_path_namespace_scoped() {
        assert_eq!(
            build_manifest_path(
                &PathBuf::from("/foo"),
                "",
                "openshift-machine-api",
                "machines",
                "machine.openshift.io"
            ),
            PathBuf::from("/foo/namespaces/openshift-machine-api/machine.openshift.io/machines")
        )
    }

    #[test]
    fn test_build_manifest_path_namespace_scoped_named_resource() {
        assert_eq!(
            build_manifest_path(
                &PathBuf::from("/foo"),
                "machine1",
                "openshift-machine-api",
                "machines",
                "machine.openshift.io"
            ),
            PathBuf::from(
                "/foo/namespaces/openshift-machine-api/machine.openshift.io/machines/machine1.yaml"
            )
        )
    }

    #[test]
    fn test_get_cluster_version() {
        assert_eq!(
            get_cluster_version(&PathBuf::from(
                "testdata/must-gather-valid/sample-openshift-release"
            )),
            "X.Y.Z-fake-test"
        )
    }

    #[test]
    fn test_get_nodes() {
        assert_eq!(
            get_nodes(&PathBuf::from(
                "testdata/must-gather-valid/sample-openshift-release"
            ))
            .len(),
            2
        )
    }
}
