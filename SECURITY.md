# Security policy

## Supported versions

shellcheck-rust has no stable release yet. Security fixes go to the `main`
branch and to the latest release, once there is one.

## What counts as a security issue

shellcheck-rust reads untrusted shell scripts, and many people run it in CI
on code from pull requests. We treat these as security issues:

- Memory-safety bugs. The project has no `unsafe` code, but dependencies might
  still have them.
- An input that makes the linter hang, or use unbounded memory or stack, in a
  way that a CI job's own timeout would not reasonably contain. The
  pathological-input tests are meant to catch these.
- Anything that makes the linter **execute**, write or delete files, or reach
  the network. shellcheck-rust only reads the files you pass to it, the files
  they `source` (with `-x` / `--external-sources`) and its configuration files.
- A vulnerability in a release artifact or in the release process.

Wrong or missing warnings are **not** security issues, even for security
checks such as SC2086. Report those as normal issues with the
[false positive / false negative](.github/ISSUE_TEMPLATE/false-positive-negative.yml)
template.

## Reporting a vulnerability

Please **do not open a public issue.**

- Preferred: use GitHub's private vulnerability reporting.
  (Repository **Security** tab, then **Report a vulnerability**.)
- Or email: <!-- CONTACT: add a security contact email before going public -->[SECURITY CONTACT EMAIL]

Please include the input that triggers the problem, the exact command line,
the version (`shellcheck-rust --version`) and the platform.

We aim to acknowledge reports within 7 days and to agree on a disclosure date
with the reporter. If ShellCheck has the same issue, we will coordinate with
upstream before disclosing it publicly.

## Upstream

This project is an unofficial port of ShellCheck. Report vulnerabilities in
ShellCheck itself (the Haskell implementation) to the ShellCheck project, not
here.
