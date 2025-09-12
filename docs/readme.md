# canopus

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

`canopus` should be able to spot some errors not handled by Github, like
detecting dangling glob patterns (i.e., the ones won't match any project path)
and more.

For instance, this is a Pull Request preview for a `CODEOWNERS` change
for this project, which should be an error since there is no `.samples`
path around:

![canopus](assets/canopus-demo.png)

In addition to that, `canopus` brings configuration options to encourage good
practices on code ownership, especially on large multimodular projects or monorepos.

`canopus` is distributed as a self-contained binary compatible with macOS and Linux,
and also through Docker. This code adheres to the
[MIT license](https://choosealicense.com/licenses/mit)

## Prior art

`canopus` takes as inspiration a couple of similar projects:

- [toptal/codeowners-checker](https://github.com/toptal/codeowners-checker)
- [mszostok/codeowners-validator](https://github.com/mszostok/codeowners-validator)
- [topfreegames/codeowners-verifier](https://github.com/topfreegames/codeowners-verifier)

Thanks to everyone that worked on this problem before ❤️
