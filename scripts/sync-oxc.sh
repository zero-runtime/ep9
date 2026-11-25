#!/usr/bin/env bash
# Syncs third_party/oxc to the commit in third_party/oxc.commit (the upstream commit of the
# crates.io oxc_parser 0.158.0 package), plus third_party/patches/oxc/*.patch (the published
# manifest and the layer-1 recursion check of H01). The root, fuzz, spikes/spike-b and
# spikes/spike-c workspaces build oxc_parser from third_party/oxc/crates/oxc_parser through their
# `[patch.crates-io]` path entry, so every cargo command there needs it first (a missing patch
# path fails the resolve step), as for scripts/sync-regress.sh. CI runs it in
# .github/actions/setup-rust.
# A second run with the same pin and patches does nothing.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
. "$root/scripts/sync-lib.sh"
sync_dep oxc https://github.com/oxc-project/oxc.git
echo "oxc: $(tr -d '[:space:]' < "$root/third_party/oxc.commit") in $root/third_party/oxc"
