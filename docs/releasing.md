# Releasing new versions

## Preparing the release

- Understand what changes we are shipping by inspecting unreleased commits

```bash
git checkout main
git pull origin main --rebase
git log $(git describe --abbrev=0 --tags)...HEAD --oneline
```

- Determine next release version according to [SemVer](https://semver.org/)
- Create a short-lived branch

```bash
git checkout -b ufs/release-x.y.z
```

- Add **notable changes** the to [changelog file](https://github.com/dotanuki-labs/canopus/blob/main/docs/changelog.md)
- Bump version at [Cargo.toml](https://github.com/dotanuki-labs/canopus/blob/main/Cargo.toml#L3)
- Commit your changes
- Ensure everything is ready by running :

```bash
cargo publish -p canopus --dry-run
```

- Raise a PR preparing the release

## Creating a release (GitHub admins-only)

- Ensure the next release is prepared (as described above)
- Execute the [CD Workflow](https://github.com/dotanuki-labs/canopus/actions/workflows/cd.yml)
- Go to the [releases page](https://github.com/dotanuki-labs/canopus/releases)
- Review the release draft and add final touches (e.g. updating `RenovateBot` identity name)
- Publish the release 🚀

## Updating distributions

- Clone [dotanuki-labs/homebrew-taps](https://github.com/dotanuki-labs/homebrew-taps)
- Create a branch like `ufs/canopus-x.y.z`
- Update the `canopus.rb` formula with proper version and checksums
- Raise a PR
