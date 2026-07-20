#!/usr/bin/env bash

set -euo pipefail

missing=""

while IFS= read -r file; do
    first_line=$(sed -n '1p' "$file")
    second_line=$(sed -n '2p' "$file")

    if [[ ! "$first_line" =~ ^//\ Copyright\ (19|20)[0-9]{2}\ the\ Underwood\ Authors$ ]] ||
        [[ "$second_line" != "// SPDX-License-Identifier: Apache-2.0 OR MIT" ]]; then
        missing="${missing}${file}"$'\n'
    fi
done < <(rg --files -g '*.rs' -g '!target/**')

if [[ -n "$missing" ]]; then
    echo "The following Rust files lack the required header:"
    echo "$missing"
    echo "Expected:"
    echo "// Copyright $(date +%Y) the Underwood Authors"
    echo "// SPDX-License-Identifier: Apache-2.0 OR MIT"
    exit 1
fi

echo "All Rust files have correct copyright headers."
