#!/usr/bin/env bash
set -euo pipefail

workflow=".github/workflows/update-homebrew-tap.yml"

if rg -q "ref: .*workflow_run\.head_sha|ref: .*format\\('refs/tags" "${workflow}"; then
  echo "workflow must not check out the release commit or tag" >&2
  exit 1
fi

if ! rg -q "RELEASE_SHA: \\$\\{\\{ github.event.workflow_run.head_sha" "${workflow}"; then
  echo "workflow should use workflow_run.head_sha only as tag-resolution data" >&2
  exit 1
fi

if ! rg -q 'git describe --tags --exact-match "\$SHA"' "${workflow}"; then
  echo "workflow should resolve the release tag from the release SHA" >&2
  exit 1
fi
