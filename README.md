# canopus

[![best practices](https://www.bestpractices.dev/projects/11177/badge)](https://www.bestpractices.dev/projects/11177)
[![DeepSource](https://app.deepsource.com/gh/dotanuki-labs/canopus.svg/?label=active+issues&show_trend=true&token=LQiIpIl6403szs6dIY1dhYkx)](https://app.deepsource.com/gh/dotanuki-labs/canopus/)[![ci](https://github.com/dotanuki-labs/gradle-wrapper-validator/actions/workflows/ci.yml/badge.svg)](https://github.com/dotanuki-labs/gradle-wrapper-validator/actions/workflows/ci.yml)
[![dependencies](https://deps.rs/crate/canopus/0.1.0/status.svg)](https://deps.rs/crate/canopus/0.1.0)
[![crates.io](https://img.shields.io/crates/v/canopus)](https://crates.io/crates/canopus)
[![ghcr.io](https://img.shields.io/badge/docker-ghcr.io-blue?logo=docker)](https://github.com/dotanuki-labs/canopus/pkgs/container/canopus)
![license](https://img.shields.io/crates/l/canopus)

> A fast and pragmatic validator for Github Codeowners

`canopus` is a small, fast and standalone CLI tool for validating
[Github Codeowners](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/about-code-owners)
written in pure Rust. It should be fast enough to be configured as a
[Git hook](https://git-scm.com/book/en/v2/Customizing-Git-Git-Hooks)
in offline mode, and eventually also in online mode.

## Main features

- local validation of `CODEOWNERS`, including additional syntax checks
- quick repairing of `CODEOWNERS`
- offline and online execution modes
- sensible opt-ins for better `CODEOWNERS` structure

Please check our [documentation](https://dotanuki-labs.github.io/canopus/)
to learn more!

## License

Copyright © 2025 — Dotanuki Labs - [The MIT license](https://choosealicense.com/licenses/mit)
