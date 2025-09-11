// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

use ignore::WalkBuilder;
use std::path::{Path, PathBuf};

pub trait DirWalking {
    fn walk(&self, origin: &Path) -> Vec<PathBuf>;
}

pub enum PathWalker {
    GitAware,

    // We opt for a define test doubles with test-only
    // visibility, pattern-matching them when needed
    #[cfg(test)]
    FakePaths(Vec<String>),
}

impl DirWalking for PathWalker {
    fn walk(&self, origin: &Path) -> Vec<PathBuf> {
        match self {
            PathWalker::GitAware => {
                let current_dir = std::env::current_dir().unwrap();

                WalkBuilder::new(origin)
                    .hidden(false)
                    .git_exclude(true)
                    .filter_entry(|entry| !entry.path().to_string_lossy().contains(".git/"))
                    .build()
                    .filter_map(|entry| entry.ok())
                    .map(|entry| {
                        // We have to check whether this is sufficient
                        if let Ok(normalized) = entry.path().to_path_buf().strip_prefix(&current_dir) {
                            normalized.to_path_buf()
                        } else {
                            entry.path().to_path_buf()
                        }
                    })
                    .collect::<Vec<_>>()
            },
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
