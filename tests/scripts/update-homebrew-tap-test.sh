#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
tmpdir="$(mktemp -d)"
trap 'rm -rf "${tmpdir}"' EXIT

bindir="${tmpdir}/bin"
log="${tmpdir}/commands.log"
mkdir -p "${bindir}"
touch "${log}"

cat > "${bindir}/curl" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
echo "curl $*" >> "${CALEPIN_TEST_LOG}"
output=""
while (($#)); do
  if [[ "$1" == "-o" ]]; then
    output="$2"
    shift 2
  else
    shift
  fi
done
if [[ -n "$output" ]]; then
  printf 'source archive\n' > "$output"
fi
STUB
chmod +x "${bindir}/curl"

cat > "${bindir}/sha256sum" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
echo "sha256sum $*" >> "${CALEPIN_TEST_LOG}"
printf 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa  %s\n' "$1"
STUB
chmod +x "${bindir}/sha256sum"

cat > "${bindir}/git" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
echo "git $*" >> "${CALEPIN_TEST_LOG}"
case "$1" in
  clone)
    mkdir -p "$3"
    ;;
  diff)
    exit 1
    ;;
  describe)
    printf 'v0.0.0\n'
    ;;
esac
STUB
chmod +x "${bindir}/git"

PATH="${bindir}:${PATH}" \
CALEPIN_TEST_LOG="${log}" \
CALEPIN_RELEASE_TAG="v9.8.7" \
GITHUB_REF_NAME="main" \
HOMEBREW_TAP_GITHUB_TOKEN="test-token" \
  bash "${repo_root}/scripts/update-homebrew-tap.sh"

if ! rg -q "calepin/archive/refs/tags/v9.8.7.tar.gz" "${log}"; then
  echo "expected source URL to use CALEPIN_RELEASE_TAG" >&2
  cat "${log}" >&2
  exit 1
fi

if ! rg -q "git commit -m Update calepin formula to v9.8.7" "${log}"; then
  echo "expected tap commit to use CALEPIN_RELEASE_TAG" >&2
  cat "${log}" >&2
  exit 1
fi
