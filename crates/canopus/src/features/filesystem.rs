// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

use ignore::WalkBuilder;
use std::path::PathBuf;

pub trait PathWalker {
    fn walk(&self) -> Vec<PathBuf>;
}

pub struct GitAwarePathWalker {
    origin: PathBuf,
}

impl GitAwarePathWalker {
    pub fn new(origin: PathBuf) -> Self {
        Self { origin }
    }
}

impl PathWalker for GitAwarePathWalker {
    fn walk(&self) -> Vec<PathBuf> {
        WalkBuilder::new(&self.origin)
            .build()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path().to_path_buf())
            .collect::<Vec<_>>()
    }
}

#[cfg(test)]
pub mod helpers {
    use crate::features::filesystem::PathWalker;
    use std::path::PathBuf;

    pub struct FakePathWalker {
        raw_paths: Vec<String>,
    }

    impl FakePathWalker {
        pub fn no_op() -> Self {
            Self::new(&[])
        }

        pub fn new(raw: &[&str]) -> Self {
            Self {
                raw_paths: raw.iter().map(|s| s.to_string()).collect(),
            }
        }
    }

    impl PathWalker for FakePathWalker {
        fn walk(&self) -> Vec<PathBuf> {
            self.raw_paths.clone().into_iter().map(PathBuf::from).collect()
        }
    }
}
