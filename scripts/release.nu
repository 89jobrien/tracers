#!/usr/bin/env nu
# Thin wrapper around the shared release pipeline in nu_libs — see
# ~/dev/nu_libs/lib/rust/taskit-release.nu for the actual implementation
# (single source of truth, reusable across any taskit-managed Rust repo).
#
# Order: validate (taskit ci --fail-fast) -> bump version from conventional
# commits since the last tag -> changelog via git-cliff -> commit+tag ->
# push --no-verify -> GitHub release -> crates.io publish -> log to
# .ctx/logs/releases.log.
#
# Usage:
#   nu scripts/release.nu --dry-run    # print every step, run nothing irreversible
#   nu scripts/release.nu              # interactive — confirms before push/release/publish
#   nu scripts/release.nu --yes        # skip the confirmation prompt

use ~/dev/nu_libs/lib/rust/taskit-release.nu *

def main [--dry-run, --yes] {
    if $dry_run {
        taskit-release --dry-run
    } else if $yes {
        taskit-release --yes
    } else {
        taskit-release
    }
}
