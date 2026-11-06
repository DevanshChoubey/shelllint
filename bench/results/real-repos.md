# Real-repo benchmark: Haskell ShellCheck vs shellcheck-rust

Generated 2026-11-06T17:56:07Z by `bench/run_real.sh`. Raw data: `bench/results/real-repos.json`.

## Machine

- CPU: 12th Gen Intel(R) Core(TM) i5-12600K (16 logical CPUs), RAM 91.8 GB
- Kernel 7.0.0-42-generic, Ubuntu 26.04.1 LTS
- Haskell: `version: v0.11.0-171-ge11f964` (`koalaman/shellcheck:latest`, static binary)
- Rust: `shellcheck-rust - shell script analysis tool (a Rust port of ShellCheck)
version: 0.0.1 (ports ShellCheck v0.11.0-171-ge11f964)
license: GNU General Public License, version 3
website: https://www.shellcheck.net`, release build (LTO, codegen-units=1), 5793440 bytes
- rustc 1.93.1 (01f6ddf75 2026-02-11) (built from a source tarball), Python 3.14.4
- Load average (1 min) at start 6.84, at end 7.42. Other agents share this machine, so timings carry noise.
- Caps: RLIMIT_AS 6000000 KiB per process, timeout 300s. sequential, one process per file, 3 passes, median of pass totals.
- Flags: `-f json1 --norc -o all` (json1 on stdout, shell from shebang)

## Summary

- 920 files, 122416 lines across 7 repos
- Total wall time (sum of per-repo medians): Haskell 99.47 s, Rust 25.88 s, speedup 3.8x
- Diagnostics identical on 815 of 918 comparable files (88.8%), compared on (line, column, code)

## Per repo

| repo | files | lines | Haskell time (s) | Rust time (s) | speedup | Haskell peak RSS (MB) | Rust peak RSS (MB) | identical files |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| nvm | 438 | 26580 | 43.67 | 12.26 | 3.6x | 3909.8 | 595.1 | 388/437 (88.8%) |
| neofetch | 1 | 11592 | 2.60 | 0.41 | 6.3x | 520.3 | 42.9 | 0/1 (0.0%) |
| acme.sh | 294 | 67183 | 46.78 | 10.51 | 4.5x | 3909.9 | 268.8 | 261/293 (89.1%) |
| bats-core | 69 | 5874 | 2.81 | 1.03 | 2.7x | 62.4 | 9.3 | 62/69 (89.9%) |
| asdf | 28 | 757 | 0.47 | 0.25 | 1.8x | 19.5 | 7.9 | 28/28 (100.0%) |
| rbenv | 31 | 1742 | 0.82 | 0.37 | 2.2x | 22.5 | 8.1 | 31/31 (100.0%) |
| pyenv | 59 | 8688 | 2.33 | 1.05 | 2.2x | 38.6 | 8.9 | 45/59 (76.3%) |

Run statuses that were not `ok` (timeouts, crashes, exit codes other than 0/1):

- nvm hs: {'exit 251': 1}
- acme.sh hs: {'exit 251': 1}

## Disagreements

| SC code | reported only by Haskell | reported only by Rust |
|---|---:|---:|
| SC2249 | 204 | 204 |

Files with any difference (`only_hs` = Haskell only, `only_rs` = Rust only):

- `nvm/.github/scripts/docker-image.sh`
  - only_hs: 68:3 SC2249
  - only_rs: 68:8 SC2249
- `nvm/bash_completion`
  - only_hs: 18:3 SC2249, 23:5 SC2249, 50:5 SC2249
  - only_rs: 18:8 SC2249, 23:10 SC2249, 50:10 SC2249
- `nvm/install.sh`
  - only_hs: 386:3 SC2249, 402:3 SC2249, 506:5 SC2249
  - only_rs: 386:8 SC2249, 402:8 SC2249, 506:10 SC2249
- `nvm/release-notes.sh`
  - only_hs: 62:3 SC2249
  - only_rs: 62:8 SC2249
- `nvm/test/fast/Aliases/an alias can store --lts`
  - only_hs: 7:3 SC2249
  - only_rs: 7:8 SC2249
- `nvm/test/fast/Aliases/an alias can store --only`
  - only_hs: 7:3 SC2249
  - only_rs: 7:8 SC2249
- `nvm/test/fast/Aliases/nvm_print_formatted_alias respects NVM_HAS_COLORS in pipeline`
  - only_hs: 19:1 SC2249
  - only_rs: 19:6 SC2249
- `nvm/test/fast/Running 'nvm --help' documents current and the .nvmrc fallback`
  - only_hs: 36:1 SC2249
  - only_rs: 36:6 SC2249
- `nvm/test/fast/Running 'nvm exec' and 'nvm run' without a resolvable version warn and fall back`
  - only_hs: 42:1 SC2249, 59:1 SC2249
  - only_rs: 42:6 SC2249, 59:6 SC2249
- `nvm/test/fast/Running 'nvm use' and 'nvm deactivate' with hashing disabled`
  - only_hs: 27:1 SC2249, 37:1 SC2249
  - only_rs: 27:6 SC2249, 37:6 SC2249
- `nvm/test/fast/Running 'nvm use' does not leave a stale hashed node`
  - only_hs: 27:1 SC2249
  - only_rs: 27:6 SC2249
- `nvm/test/fast/Sourcing nvm.sh without NVM_DIR should not use the working directory`
  - only_hs: 46:1 SC2249
  - only_rs: 46:6 SC2249
- `nvm/test/fast/Subcommands with missing or invalid args show a focused usage error`
  - only_hs: 40:3 SC2249
  - only_rs: 40:8 SC2249
- `nvm/test/fast/Unit tests/LTS status and default packages handle backslashes in NVM_DIR`
  - only_hs: 7:3 SC2249
  - only_rs: 7:8 SC2249
- `nvm/test/fast/Unit tests/bash_completion lists alias and version names without expanding them`
  - only_hs: 34:5 SC2249, 68:5 SC2249
  - only_rs: 34:10 SC2249, 68:10 SC2249
- `nvm/test/fast/Unit tests/failed checks propagate their exit codes`
  - only_hs: 7:3 SC2249
  - only_rs: 7:8 SC2249
- `nvm/test/fast/Unit tests/mirror URL credentials are masked in output`
  - only_hs: 16:3 SC2249, 49:1 SC2249
  - only_rs: 16:8 SC2249, 49:6 SC2249
- `nvm/test/fast/Unit tests/nvm commands ignore option variables set by the caller`
  - only_hs: 7:3 SC2249, 30:3 SC2249
  - only_rs: 7:8 SC2249, 30:8 SC2249
- `nvm/test/fast/Unit tests/nvm debug does not read stdin to find the shell version`
  - only_hs: 6:3 SC2249
  - only_rs: 6:8 SC2249
- `nvm/test/fast/Unit tests/nvm install validates an inherited NVM_MAKE_JOBS`
  - only_hs: 30:1 SC2249, 53:1 SC2249
  - only_rs: 30:6 SC2249, 53:6 SC2249
- `nvm/test/fast/Unit tests/nvm install verifies the installed version`
  - only_hs: 65:1 SC2249
  - only_rs: 65:6 SC2249
- `nvm/test/fast/Unit tests/nvm ls with nothing installed`
  - only_hs: 7:3 SC2249
  - only_rs: 7:8 SC2249
- `nvm/test/fast/Unit tests/nvm ls-remote aliases`
  - only_hs: 66:1 SC2249, 87:1 SC2249
  - only_rs: 66:6 SC2249, 87:6 SC2249
- `nvm/test/fast/Unit tests/nvm reinstall-packages prints no shell variables`
  - only_hs: 7:3 SC2249
  - only_rs: 7:8 SC2249
- `nvm/test/fast/Unit tests/nvm uninstall only removes aliases to the uninstalled version`
  - only_hs: 7:3 SC2249
  - only_rs: 7:8 SC2249
- `nvm/test/fast/Unit tests/nvm use and nvm which explain an excluded alpha`
  - only_hs: 7:3 SC2249
  - only_rs: 7:8 SC2249
- `nvm/test/fast/Unit tests/nvm_alias path traversal`
  - only_hs: 7:3 SC2249, 46:3 SC2249
  - only_rs: 7:8 SC2249, 46:8 SC2249
- `nvm/test/fast/Unit tests/nvm_auto use with a prerelease default`
  - only_hs: 7:3 SC2249
  - only_rs: 7:8 SC2249
- `nvm/test/fast/Unit tests/nvm_download_artifact version injection`
  - only_hs: 41:1 SC2249
  - only_rs: 41:6 SC2249
- `nvm/test/fast/Unit tests/nvm_filter_installed_versions`
  - only_hs: 7:3 SC2249
  - only_rs: 7:8 SC2249
- `nvm/test/fast/Unit tests/nvm_get_os`
  - only_hs: 50:3 SC2249
  - only_rs: 50:8 SC2249
- `nvm/test/fast/Unit tests/nvm_ls alpha`
  - only_hs: 7:3 SC2249
  - only_rs: 7:8 SC2249
- `nvm/test/fast/Unit tests/nvm_ls lists every installed prerelease`
  - only_hs: 7:3 SC2249
  - only_rs: 7:8 SC2249
- `nvm/test/fast/Unit tests/nvm_ls matches a prerelease only at a dot`
  - only_hs: 7:3 SC2249
  - only_rs: 7:8 SC2249
- `nvm/test/fast/Unit tests/nvm_ls with only a system version`
  - only_hs: 7:3 SC2249
  - only_rs: 7:8 SC2249
- `nvm/test/fast/Unit tests/nvm_ls_cached alpha`
  - only_hs: 7:3 SC2249
  - only_rs: 7:8 SC2249
- `nvm/test/fast/Unit tests/nvm_ls_remote alpha`
  - only_hs: 7:3 SC2249
  - only_rs: 7:8 SC2249
- `nvm/test/fast/Unit tests/nvm_ls_remote invalid versions`
  - only_hs: 22:3 SC2249
  - only_rs: 22:8 SC2249
- `nvm/test/fast/Unit tests/nvm_print_color_legend`
  - only_hs: 30:3 SC2249, 97:1 SC2249, 116:1 SC2249, 122:1 SC2249
  - only_rs: 30:8 SC2249, 97:6 SC2249, 116:6 SC2249, 122:6 SC2249
- `nvm/test/fast/Unit tests/nvm_process_nvmrc does not show a symlinked .nvmrc's content`
  - only_hs: 29:3 SC2249, 59:3 SC2249
  - only_rs: 29:8 SC2249, 59:8 SC2249
- `nvm/test/fast/Unit tests/nvm_record_lts_status`
  - only_hs: 7:3 SC2249
  - only_rs: 7:8 SC2249
- `nvm/test/fast/Unit tests/nvm_remote_version pattern starting with a dash`
  - only_hs: 36:3 SC2249, 56:3 SC2249
  - only_rs: 36:8 SC2249, 56:8 SC2249
- `nvm/test/fast/Unit tests/nvm_sanitize_auth_header`
  - only_hs: 32:1 SC2249
  - only_rs: 32:6 SC2249
- `nvm/test/fast/Unit tests/release-notes.sh replaces control characters in commit subjects`
  - only_hs: 74:3 SC2249
  - only_rs: 74:8 SC2249
- `nvm/test/fast/Unit tests/security_wget_injection`
  - only_hs: 36:1 SC2249
  - only_rs: 36:6 SC2249
- `nvm/test/fixtures/fake-source-tarball/configure`
  - only_hs: 10:3 SC2249
  - only_rs: 10:8 SC2249
- `nvm/test/install_script/install_nvm_as_script_download_failure`
  - only_hs: 26:3 SC2249
  - only_rs: 26:8 SC2249
- `nvm/test/installation_iojs/install from source`
  - only_hs: 20:1 SC2249
  - only_rs: 20:6 SC2249
- `nvm/test/installation_iojs/install version specified in .nvmrc from source`
  - only_hs: 19:1 SC2249
  - only_rs: 19:6 SC2249
- `neofetch/neofetch`
  - only_hs: 972:5 SC2249, 1031:17 SC2249, 1045:17 SC2249, 1051:17 SC2249, 1129:17 SC2249, 1163:13 SC2249, 1167:21 SC2249, 1236:5 SC2249, 1268:13 SC2249, 1376:5 SC2249, 1411:5 SC2249, 1426:5 SC2249, 1489:5 SC2249, 1546:5 SC2249, 1621:13 SC2249, 1671:13 SC2249, 1710:5 SC2249, 1797:13 SC2249, 1845:5 SC2249, 1858:9 SC2249, 1964:9 SC2249, 2028:5 SC2249, 2190:5 SC2249, 2257:21 SC2249, 2272:21 SC2249, 2288:13 SC2249, 2295:13 SC2249, 2356:13 SC2249, 2380:13 SC2249, 2408:13 SC2249, 2604:13 SC2249, 2677:5 SC2249, 2682:17 SC2249, 2759:13 SC2249, 2804:5 SC2249, 2821:5 SC2249, 2939:13 SC2249, 3083:17 SC2249, 3136:9 SC2249, 3356:5 SC2249, 3616:13 SC2249, 3709:9 SC2249, 3746:9 SC2249, 3763:5 SC2249, 3774:21 SC2249, 3788:13 SC2249, 3843:5 SC2249, 3851:5 SC2249, 3936:5 SC2249, 4134:13 SC2249, 4433:5 SC2249, 4637:5 SC2249, 4642:5 SC2249, 4861:5 SC2249, 4906:13 SC2249, 5244:9 SC2249, 11375:13 SC2249
  - only_rs: 972:10 SC2249, 1031:22 SC2249, 1045:22 SC2249, 1051:22 SC2249, 1129:22 SC2249, 1163:18 SC2249, 1167:26 SC2249, 1236:10 SC2249, 1268:18 SC2249, 1376:10 SC2249, 1411:10 SC2249, 1426:10 SC2249, 1489:10 SC2249, 1546:10 SC2249, 1621:18 SC2249, 1671:18 SC2249, 1710:10 SC2249, 1797:18 SC2249, 1845:10 SC2249, 1858:14 SC2249, 1964:14 SC2249, 2028:10 SC2249, 2190:10 SC2249, 2257:26 SC2249, 2272:26 SC2249, 2288:18 SC2249, 2295:18 SC2249, 2356:18 SC2249, 2380:18 SC2249, 2408:18 SC2249, 2604:18 SC2249, 2677:10 SC2249, 2682:22 SC2249, 2759:18 SC2249, 2804:10 SC2249, 2821:10 SC2249, 2939:18 SC2249, 3083:22 SC2249, 3136:14 SC2249, 3356:10 SC2249, 3616:18 SC2249, 3709:14 SC2249, 3746:14 SC2249, 3763:10 SC2249, 3774:26 SC2249, 3788:18 SC2249, 3843:10 SC2249, 3851:10 SC2249, 3936:10 SC2249, 4134:18 SC2249, 4433:10 SC2249, 4637:10 SC2249, 4642:10 SC2249, 4861:10 SC2249, 4906:18 SC2249, 5244:14 SC2249, 11375:18 SC2249
- `acme.sh/deploy/proxmoxbs.sh`
  - only_hs: 126:3 SC2249
  - only_rs: 126:8 SC2249
- `acme.sh/deploy/proxmoxve.sh`
  - only_hs: 138:3 SC2249
  - only_rs: 138:8 SC2249
- `acme.sh/deploy/redfish.sh`
  - only_hs: 346:7 SC2249
  - only_rs: 346:12 SC2249
- `acme.sh/deploy/truenas_websocat.sh`
  - only_hs: 133:3 SC2249, 261:7 SC2249, 382:3 SC2249
  - only_rs: 133:8 SC2249, 261:12 SC2249, 382:8 SC2249
- `acme.sh/deploy/truenas_ws.sh`
  - only_hs: 49:3 SC2249, 104:3 SC2249, 207:5 SC2249
  - only_rs: 49:8 SC2249, 104:8 SC2249, 207:10 SC2249
- `acme.sh/deploy/unifios.sh`
  - only_hs: 122:5 SC2249
  - only_rs: 122:10 SC2249
- `acme.sh/dnsapi/dns_aws.sh`
  - only_hs: 249:9 SC2249
  - only_rs: 249:14 SC2249
- `acme.sh/dnsapi/dns_baidu.sh`
  - only_hs: 152:3 SC2249, 204:3 SC2249, 764:3 SC2249
  - only_rs: 152:8 SC2249, 204:8 SC2249, 764:8 SC2249
- `acme.sh/dnsapi/dns_bergdns.sh`
  - only_hs: 123:3 SC2249, 129:3 SC2249, 265:7 SC2249, 276:7 SC2249, 340:3 SC2249
  - only_rs: 123:8 SC2249, 129:8 SC2249, 265:12 SC2249, 276:12 SC2249, 340:8 SC2249
- `acme.sh/dnsapi/dns_bh.sh`
  - only_hs: 124:5 SC2249, 129:5 SC2249
  - only_rs: 124:10 SC2249, 129:10 SC2249
- `acme.sh/dnsapi/dns_comlaude.sh`
  - only_hs: 68:3 SC2249
  - only_rs: 68:8 SC2249
- `acme.sh/dnsapi/dns_cpanel.sh`
  - only_hs: 140:5 SC2249
  - only_rs: 140:10 SC2249
- `acme.sh/dnsapi/dns_cpanel_uapi.sh`
  - only_hs: 46:3 SC2249
  - only_rs: 46:8 SC2249
- `acme.sh/dnsapi/dns_czechia.sh`
  - only_hs: 194:5 SC2249
  - only_rs: 194:10 SC2249
- `acme.sh/dnsapi/dns_da.sh`
  - only_hs: 100:3 SC2249
  - only_rs: 100:8 SC2249
- `acme.sh/dnsapi/dns_dnsmint.sh`
  - only_hs: 155:5 SC2249, 224:5 SC2249
  - only_rs: 155:10 SC2249, 224:10 SC2249
- `acme.sh/dnsapi/dns_gname.sh`
  - only_hs: 235:7 SC2249
  - only_rs: 235:12 SC2249
- `acme.sh/dnsapi/dns_hostup.sh`
  - only_hs: 161:3 SC2249, 172:3 SC2249, 197:3 SC2249, 324:5 SC2249
  - only_rs: 161:8 SC2249, 172:8 SC2249, 197:8 SC2249, 324:10 SC2249
- `acme.sh/dnsapi/dns_laodc.sh`
  - only_hs: 152:3 SC2249
  - only_rs: 152:8 SC2249
- `acme.sh/dnsapi/dns_mvmnet.sh`
  - only_hs: 193:3 SC2249
  - only_rs: 193:8 SC2249
- `acme.sh/dnsapi/dns_nanelo.sh`
  - only_hs: 102:5 SC2249
  - only_rs: 102:10 SC2249
- `acme.sh/dnsapi/dns_nexdns.sh`
  - only_hs: 229:3 SC2249
  - only_rs: 229:8 SC2249
- `acme.sh/dnsapi/dns_optidata.sh`
  - only_hs: 66:5 SC2249, 288:7 SC2249, 394:5 SC2249, 398:9 SC2249, 424:3 SC2249
  - only_rs: 66:10 SC2249, 288:12 SC2249, 394:10 SC2249, 398:14 SC2249, 424:8 SC2249
- `acme.sh/dnsapi/dns_pblin.sh`
  - only_hs: 157:3 SC2249
  - only_rs: 157:8 SC2249
- `acme.sh/dnsapi/dns_poweradmin.sh`
  - only_hs: 134:5 SC2249, 199:7 SC2249
  - only_rs: 134:10 SC2249, 199:12 SC2249
- `acme.sh/dnsapi/dns_regru.sh`
  - only_hs: 99:5 SC2249
  - only_rs: 99:10 SC2249
- `acme.sh/dnsapi/dns_selfhost.sh`
  - only_hs: 23:3 SC2249, 103:3 SC2249
  - only_rs: 23:8 SC2249, 103:8 SC2249
- `acme.sh/notify/aws_ses.sh`
  - only_hs: 132:9 SC2249
  - only_rs: 132:14 SC2249
- `acme.sh/notify/mail.sh`
  - only_hs: 125:3 SC2249
  - only_rs: 125:8 SC2249
- `acme.sh/notify/ntfy.sh`
  - only_hs: 40:3 SC2249
  - only_rs: 40:8 SC2249
- `acme.sh/notify/smtp.sh`
  - only_hs: 105:3 SC2249
  - only_rs: 105:8 SC2249
- `acme.sh/notify/teams.sh`
  - only_hs: 30:3 SC2249
  - only_rs: 30:8 SC2249
- `bats-core/contrib/release.sh`
  - only_hs: 140:5 SC2249
  - only_rs: 140:10 SC2249
- `bats-core/contrib/semver`
  - only_hs: 329:1 SC2249
  - only_rs: 329:6 SC2249
- `bats-core/lib/bats-core/test_functions.bash`
  - only_hs: 370:3 SC2249
  - only_rs: 370:8 SC2249
- `bats-core/lib/bats-core/validator.bash`
  - only_hs: 19:7 SC2249
  - only_rs: 19:12 SC2249
- `bats-core/libexec/bats-core/bats-format-junit`
  - only_hs: 272:5 SC2249
  - only_rs: 272:10 SC2249
- `bats-core/libexec/bats-core/bats-format-pretty`
  - only_hs: 12:3 SC2249, 131:3 SC2249
  - only_rs: 12:8 SC2249, 131:8 SC2249
- `bats-core/libexec/bats-core/bats-preprocess`
  - only_hs: 104:9 SC2249
  - only_rs: 104:14 SC2249
- `pyenv/libexec/pyenv-init`
  - only_hs: 78:3 SC2249
  - only_rs: 78:8 SC2249
- `pyenv/plugins/pyenv-binary/libexec/pyenv-binary-generate-installer`
  - only_hs: 61:3 SC2249
  - only_rs: 61:8 SC2249
- `pyenv/plugins/pyenv-binary/libexec/pyenv-binary-install`
  - only_hs: 22:1 SC2249, 34:1 SC2249, 69:5 SC2249
  - only_rs: 22:6 SC2249, 34:6 SC2249, 69:10 SC2249
- `pyenv/plugins/pyenv-binary/libexec/pyenv-binary-package`
  - only_hs: 62:1 SC2249
  - only_rs: 62:6 SC2249
- `pyenv/plugins/pyenv-binary/libexec/pyenv-binary-package-name`
  - only_hs: 24:1 SC2249
  - only_rs: 24:6 SC2249
- `pyenv/plugins/pyenv-binary/libexec/pyenv-binary-relocate`
  - only_hs: 52:7 SC2249, 63:5 SC2249
  - only_rs: 52:12 SC2249, 63:10 SC2249
- `pyenv/plugins/pyenv-binary/libexec/pyenv-binary-save`
  - only_hs: 64:1 SC2249, 71:1 SC2249, 79:1 SC2249
  - only_rs: 64:6 SC2249, 71:6 SC2249, 79:6 SC2249
- `pyenv/plugins/pyenv-link/bin/pyenv-link-version`
  - only_hs: 42:9 SC2249, 117:1 SC2249
  - only_rs: 42:14 SC2249, 117:6 SC2249
- `pyenv/plugins/python-build/bin/pyenv-install`
  - only_hs: 242:5 SC2249
  - only_rs: 242:10 SC2249
- `pyenv/plugins/python-build/bin/pyenv-uninstall`
  - only_hs: 42:3 SC2249
  - only_rs: 42:8 SC2249
- `pyenv/plugins/python-build/test/fixtures/fake_downloader`
  - only_hs: 9:1 SC2249
  - only_rs: 9:6 SC2249
- `pyenv/pyenv.d/exec/pip-rehash/conda`
  - only_hs: 24:3 SC2249
  - only_rs: 24:8 SC2249
- `pyenv/pyenv.d/exec/pip-rehash/pip`
  - only_hs: 25:5 SC2249
  - only_rs: 25:10 SC2249
- `pyenv/src/shobj-conf`
  - only_hs: 70:2 SC2249
  - only_rs: 70:7 SC2249
