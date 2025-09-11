# Using this tool

## Overview

The general way to use this tool is

```bash
canopus <TASK> -p/--path <PATH> [OPTIONS]
```

Although small, you may want to check the whole command line interface with

```bash
canopus --help 
```

## Validating a `CODEOWNERS` file

To validate your `CODEOWNERS` run

```bash
canopus validate -p <project-root> 
```

**canopus** will look for a single `CODEOWNERS` file in the following convention paths:

- `<project-root>/.github/CODEOWNERS`
- `<project-root>/docs/CODEOWNERS`
- `<project-root>/CODEOWNERS`

and report the following issues:

| **Issue Alias**                    | **Category**           | **Offline Check** |
|------------------------------------|------------------------|-------------------|
| InvalidSyntax                      | Structural Consistency | Yes               |
| DanglingGlobPattern                | Structural Consistency | Yes               |
| DuplicateOwnership                 | Structural Consistency | Yes               |
| CannotListMembersInTheOrganization | Github Consistency     | No                |
| CannotVerifyUser                   | Github Consistency     | No                |
| CannotVerifyTeam                   | Github Consistency     | No                |
| OrganizationDoesNotExist           | Github Consistency     | No                |
| TeamDoesNotMatchOrganization       | Github Consistency     | Yes               |
| TeamDoesNotExist                   | Github Consistency     | No                |
| OutsiderUser                       | Github Consistency     | No                |
| UserDoesNotExist                   | Github Consistency     | No                |
| EmailOwnerForbidden                | Custom Configuration   | Yes               |
| OnlyGithubTeamOwnerAllowed         | Custom Configuration   | Yes               |
| OnlyOneOwnerPerEntry               | Custom Configuration   | Yes               |

> [!WARNING]
>
> To perform online checks, `canopus` expects a `GITHUB_TOKEN` environment variable to be set.
> The identity used to issue such a Github PAT must have access to the organization defined by
> `canopus.toml`

Online checks use Github REST v3 API, hitting the following endpoints:

- `GET /users/{user-handle}`
- `GET /orgs/{org-handle}/members`
- `GET /orgs/{org-handle}/teams/{team-handle}`

## Repairing a `CODEOWNERS` file

To repair your `CODEOWNERS` configuration run

```bash
canopus repair -p <project-root>
```

**canopus** will patch the existing `CODEOWNERS` file in-place.

To preview which lines will be repaired:

```bash
canopus repair -p <project-root> --dry-run
```

By default, **canopus** will preserve broken `CODEOWNERS` entries by commenting them

```gitignore
*.rs      @dotanuki/crabbers
# *.js    dotanuki/frontend (preserved by canopus)
```

To delete broken `CODEOWNERS` entries instead

```bash
canopus repair -p <project-root> --remove-lines
```

## Configuring the execution

This tool expects a `<project-root>/.github/canopus.toml` to exist.

This file supports the following options:

```toml
[general]
github-organization = "dotanuki-labs"   # Mandatory
offline-checks-only = false             # Optional (default : false)

[ownership]
forbid-email-owners = true              # Optional (default : false)
enforce-github-teams-owners = false     # Optional (default : false)
enforce-one-owner-per-line = false      # Optional (default : false)
```

For large projects managed by multiple teams and leveraging an extensive `CODEOWNERS`
setting, the general advice is having a `canopus.toml` like:

```toml
[general]
github-organization = "dotanuki-labs"
offline-checks-only = false

[ownership]
enforce-github-teams-owners = true
enforce-one-owner-per-line = true
```

Note that setting `true` to `enforce-github-teams-owners` overrides
`forbid-email-owners`. In addition to that, `enforce-one-owner-per-line`
should help promoting well-defined ownership across project modules, especially on top
of a Github teams management that praises the
[Conway's Law](https://en.wikipedia.org/wiki/Conway%27s_law)
