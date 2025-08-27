// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

use ignore::WalkBuilder;
use std::path::PathBuf;

pub trait DirWalking {
    fn walk(&self) -> Vec<PathBuf>;
}

pub enum PathWalker {
    GitAware(PathBuf),
    #[cfg(test)]
    FakePaths(Vec<String>),
}

impl DirWalking for PathWalker {
    fn walk(&self) -> Vec<PathBuf> {
        match self {
            PathWalker::GitAware(origin) => WalkBuilder::new(origin)
                .build()
                .filter_map(|entry| entry.ok())
                .map(|entry| entry.path().to_path_buf())
                .collect::<Vec<_>>(),
            #[cfg(test)]
            PathWalker::FakePaths(paths) => paths.clone().into_iter().map(PathBuf::from).collect(),
        }
    }
}

impl PathWalker {
    #[cfg(test)]
    pub fn with_paths(paths: Vec<&str>) -> Self {
        PathWalker::FakePaths(paths.into_iter().map(String::from).collect())
    }
}
