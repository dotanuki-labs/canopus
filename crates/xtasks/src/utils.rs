// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

use crate::utils::BuildEnvironment::{CI, Local};
use std::env;

static CALLINECTES_DOCKER_IMAGE: &str = "ghcr.io/dotanuki-labs/callinectes:latest";
static CALLINECTES_DOCKER_DIGEST: &str = "f5f720d0f61313bb1687a9f64d5dca3d310645d6c48cdea70156f5ef7f29e3b3";

static ENV_VAR_RUNNING_ON_CI: &str = "CI";

pub enum BuildEnvironment {
    CI,
    Local,
}

pub fn evaluate_build_environment() -> BuildEnvironment {
    match env::var(ENV_VAR_RUNNING_ON_CI) {
        Ok(_) => CI,
        Err(_) => Local,
    }
}

pub fn docker_execution_arguments() -> (String, String) {
    let docker_package = format!("{CALLINECTES_DOCKER_IMAGE}@sha256:{CALLINECTES_DOCKER_DIGEST}");
    let current_dir = env::current_dir().expect("Failed to get current directory");
    let pwd = current_dir
        .to_str()
        .expect("Failed to convert current directory to string");
    (format!("{pwd}:/usr/src"), docker_package)
}
