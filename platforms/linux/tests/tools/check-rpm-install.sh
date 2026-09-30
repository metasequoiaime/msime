#!/usr/bin/env bash
# Install a built RPM into a clean Fedora container and remove it again: the check #2095 needed. An RPM whose Requires name libraries or symbol versions Fedora does not provide (Debian's libcurl CURL_OPENSSL_4, boost 1.83) builds and lints fine and fails only here, at dnf's dependency resolution.
#
# It also asserts that the package requires none of the libraries it carries in its private directory, which rpmbuild would otherwise turn into unsatisfiable Requires, and runs the maintainer scripts through a real install and erase.
#
# Usage: platforms/linux/tests/tools/check-rpm-install.sh <package.rpm>
set -euo pipefail

rpm_path=${1:?usage: check-rpm-install.sh <package.rpm>}
[ -f "$rpm_path" ] || { echo "no such file: $rpm_path" >&2; exit 2; }
dir=$(cd "$(dirname "$rpm_path")" && pwd)
name=$(basename "$rpm_path")

# The same Fedora release Dockerfile.package-rpm builds on.
image=$(sed -n 's/^FROM \(fedora:[^ ]*\).*/\1/p' "$(dirname "$0")/Dockerfile.package-rpm")
[ -n "$image" ] || { echo "could not read the Fedora image from Dockerfile.package-rpm" >&2; exit 2; }

docker run --rm -v "$dir":/dist:ro "$image" bash -euo pipefail -c '
  requires=$(rpm -qpR "/dist/$1")
  echo "$requires"
  if grep -E "libmsime_host_api|libsherpa-onnx|libonnxruntime|CURL_OPENSSL_4|libboost" <<<"$requires"; then
    echo "the package requires a library it carries privately or one only Debian provides" >&2
    exit 1
  fi
  dnf install -y --setopt=install_weak_deps=False "/dist/$1"
  rpm -q msime-linux
  test -x /usr/bin/msime-linux-setup
  dnf remove -y msime-linux
  ! rpm -q msime-linux
  echo "rpm install check passed: $1"
' _ "$name"
