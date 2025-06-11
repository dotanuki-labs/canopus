// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

use assert_cmd::Command;
use predicates::str::contains;

fn sut() -> Command {
    Command::cargo_bin("canopus").expect("Should be able to create a command")
}

#[test]
fn should_parse_arguments() {
    let execution = sut().args(["--name", "John"]).assert();

    let expected = "Hello, John!\n";
    execution.stdout(expected);
}

#[test]
fn should_show_help() {
    let description = "Usage: canopus";

    let execution = sut().arg("--help").assert();
    execution.stdout(contains(description));
}

#[test]
fn should_fail_without_arguments() {
    let instruction = "required arguments were not provided";

    let execution = sut().assert();
    execution.failure().stderr(contains(instruction));
}
