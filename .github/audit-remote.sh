#!/usr/bin/env bash
# Copyright 2026 the Underwood Authors
# SPDX-License-Identifier: Apache-2.0 OR MIT

set -euo pipefail

repo="${GITHUB_REPOSITORY:-forest-rs/underwood}"
ruleset_name="Underwood protected main"

repo_json="$(gh api "repos/${repo}")"
jq -e '
  .visibility == "public"
  and .default_branch == "main"
  and .allow_merge_commit == false
  and .allow_rebase_merge == false
  and .allow_squash_merge == true
  and .allow_auto_merge == true
  and .delete_branch_on_merge == true
  and .allow_update_branch == true
' <<<"${repo_json}" >/dev/null

actions_json="$(gh api "repos/${repo}/actions/permissions")"
jq -e '
  .enabled == true
  and .allowed_actions == "all"
  and .sha_pinning_required == true
' <<<"${actions_json}" >/dev/null

workflow_permissions_json="$(gh api "repos/${repo}/actions/permissions/workflow")"
jq -e '
  .default_workflow_permissions == "read"
  and .can_approve_pull_request_reviews == false
' <<<"${workflow_permissions_json}" >/dev/null

gh api --silent "repos/${repo}/vulnerability-alerts"

rulesets_json="$(gh api "repos/${repo}/rulesets")"
ruleset_id="$(
  jq -er --arg name "${ruleset_name}" '
    [.[] | select(.name == $name and .enforcement == "active" and .target == "branch")]
    | if length == 1 then .[0].id else error("expected exactly one active ruleset") end
  ' <<<"${rulesets_json}"
)"
ruleset_json="$(gh api "repos/${repo}/rulesets/${ruleset_id}")"

jq -e '
  .name == "Underwood protected main"
  and .enforcement == "active"
  and .target == "branch"
  and .conditions.ref_name.include == ["~DEFAULT_BRANCH"]
  and .conditions.ref_name.exclude == []
  and .bypass_actors == [{
    "actor_id": 178582,
    "actor_type": "User",
    "bypass_mode": "pull_request"
  }]
  and ([.rules[].type] | sort) == ([
    "deletion",
    "non_fast_forward",
    "required_linear_history",
    "pull_request",
    "required_status_checks",
    "merge_queue"
  ] | sort)
' <<<"${ruleset_json}" >/dev/null

jq -e '
  first(.rules[] | select(.type == "pull_request")).parameters as $pull
  | $pull.allowed_merge_methods == ["squash"]
    and $pull.dismiss_stale_reviews_on_push == true
    and $pull.require_code_owner_review == true
    and $pull.required_review_thread_resolution == true
    and $pull.required_approving_review_count == 0
' <<<"${ruleset_json}" >/dev/null

jq -e '
  first(.rules[] | select(.type == "required_status_checks")).parameters as $checks
  | $checks.strict_required_status_checks_policy == true
    and (
      [$checks.required_status_checks[].context] | sort
    ) == ([
      "formatting and text policy",
      "clippy and tests (ubuntu-latest)",
      "clippy and tests (macos-latest)",
      "clippy and tests (windows-latest)",
      "repository policy",
      "minimum supported Rust",
      "rustdoc"
    ] | sort)
' <<<"${ruleset_json}" >/dev/null

jq -e '
  first(.rules[] | select(.type == "merge_queue")).parameters as $queue
  | $queue.grouping_strategy == "ALLGREEN"
    and $queue.merge_method == "SQUASH"
    and $queue.max_entries_to_merge == 1
    and $queue.min_entries_to_merge == 1
    and $queue.check_response_timeout_minutes == 60
' <<<"${ruleset_json}" >/dev/null

if grep -REh '^[[:space:]]*uses:' .github/workflows \
  | grep -Ev '@[0-9a-f]{40}([[:space:]]|$)'; then
  echo "Every GitHub Action must be pinned to a 40-character commit SHA." >&2
  exit 1
fi

test -f .github/CODEOWNERS

echo "Underwood remote repository controls: ok"
