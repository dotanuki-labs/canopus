# canopus

> A fast and pragmatic validator for Github Codeowners

`canopus` is a small, fast and standalone CLI tool for validating
[Github Codeowners](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/about-code-owners)
written in pure Rust. Hopefully `canopus` will help troubleshooting errors like




Unlike other competing tools, `canopus` brings configuration options
to encourage good practices on code ownership, especially on large
multimodular projects or monorepos.

`canopus` should be fast enough to be configured as a 
[Git hook](https://git-scm.com/book/en/v2/Customizing-Git-Git-Hooks)
in offline mode.
