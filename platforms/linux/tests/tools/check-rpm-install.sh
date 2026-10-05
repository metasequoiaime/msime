#!/usr/bin/env bash
# Install a built RPM into a clean Fedora container and remove it again: the check #2095 needed. An RPM whose Requires name libraries or symbol versions Fedora does not provide (Debian's libcurl CURL_OPENSSL_4, boost 1.83) builds and lints fine and fails only here, at dnf's dependency resolution.
#
# It also asserts that the package requires none of the libraries it carries in its private directory, which rpmbuild would otherwise turn into unsatisfiable Requires, that its payload is xz at level 9 as packaging.cmake asks, that every installed ELF file is stripped (CPACK_STRIP_FILES for the CMake targets, CARGO_PROFILE_RELEASE_STRIP for the Rust ones; the prebuilt sherpa-onnx and ONNX Runtime libraries ship as upstream builds them and are exempt), the same check release-linux.yml runs on the .deb, and runs the maintainer scripts through a real install and erase.
#
# Usage: platforms/linux/tests/tools/check-rpm-install.sh <package.rpm>...
#
# 给出几个版本的包（msime-linux、msime-linux-<id>）时，它们一起装进同一个容器，确认能同时安装、各自的首次配置命令都在；然后逐个卸载，每卸一个都核对其余的包一个文件都没少（rpm -V 不报 missing），卸载一个版本不会带走另一个版本的东西。
set -euo pipefail

[ $# -gt 0 ] || { echo "usage: check-rpm-install.sh <package.rpm>..." >&2; exit 2; }
dir=""
names=()
for rpm_path in "$@"; do
  [ -f "$rpm_path" ] || { echo "no such file: $rpm_path" >&2; exit 2; }
  rpm_dir=$(cd "$(dirname "$rpm_path")" && pwd)
  [ -z "$dir" ] || [ "$dir" = "$rpm_dir" ] || { echo "every package must be in the same directory" >&2; exit 2; }
  dir=$rpm_dir
  names+=("$(basename "$rpm_path")")
done

# The same Fedora release Dockerfile.package-rpm builds on.
image=$(sed -n 's/^FROM \(fedora:[^ ]*\).*/\1/p' "$(dirname "$0")/Dockerfile.package-rpm")
[ -n "$image" ] || { echo "could not read the Fedora image from Dockerfile.package-rpm" >&2; exit 2; }

docker run --rm -v "$dir":/dist:ro "$image" bash -euo pipefail -c '
  packages=()
  files=()
  for file in "$@"; do
    requires=$(rpm -qpR "/dist/$file")
    echo "$requires"
    if grep -E "libmsime_host_api|libsherpa-onnx|libonnxruntime|CURL_OPENSSL_4|libboost" <<<"$requires"; then
      echo "the package requires a library it carries privately or one only Debian provides" >&2
      exit 1
    fi
    payload=$(rpm -qp --qf "%{PAYLOADCOMPRESSOR} %{PAYLOADFLAGS}" "/dist/$file")
    [ "$payload" = "xz 9" ] || { echo "$file: payload is $payload, not the xz 9 packaging.cmake sets" >&2; exit 1; }
    packages+=("$(rpm -qp --qf "%{NAME}" "/dist/$file")")
    files+=("/dist/$file")
  done
  dnf install -y --setopt=install_weak_deps=False "${files[@]}"
  for package in "${packages[@]}"; do
    rpm -q "$package"
    test -x "/usr/bin/$package-setup"
  done
  # Installed after the packages so it cannot satisfy a dependency they fail to declare.
  dnf install -y --setopt=install_weak_deps=False file
  for package in "${packages[@]}"; do
    unstripped=$(rpm -ql "$package" | while IFS= read -r path; do
      [ -f "$path" ] && [ ! -L "$path" ] || continue
      case "$(basename "$path")" in (libonnxruntime.so*|libsherpa-onnx-c-api.so*) continue ;; esac
      file "$path"
    done | grep ": *ELF" | grep -v ", stripped" || true)
    [ -z "$unstripped" ] || { echo "$package has unstripped ELF files:" >&2; echo "$unstripped" >&2; exit 1; }
  done
  for index in "${!packages[@]}"; do
    dnf remove -y "${packages[$index]}"
    if rpm -q "${packages[$index]}"; then
      echo "${packages[$index]} is still installed after dnf remove" >&2
      exit 1
    fi
    for remaining in "${packages[@]:$((index + 1))}"; do
      if rpm -V "$remaining" | grep missing; then
        echo "removing ${packages[$index]} took files of $remaining with it" >&2
        exit 1
      fi
      test -x "/usr/bin/$remaining-setup"
    done
  done
  echo "rpm install check passed: ${packages[*]}"
' _ "${names[@]}"
