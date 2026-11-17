#!/usr/bin/env bash
# 只决定签名路线；不读取或输出签名凭据。
set -euo pipefail

require_signpath() {
  local missing=()
  [[ "${TOKEN_SET:-}" == true ]] || missing+=(secrets.SIGNPATH_TOKEN)
  [[ -n "${ORGANIZATION:-}" ]] || missing+=(vars.SIGNPATH_ORGANIZATION_ID)
  [[ -n "${PROJECT:-}" ]] || missing+=(vars.SIGNPATH_PROJECT_SLUG)
  if (( ${#missing[@]} )); then echo "::error::SignPath 签名缺少配置：${missing[*]}"; exit 1; fi
}

signpath=false
simplysign=false
policy=''
if [[ "${TEST:-false}" == true ]]; then
  require_signpath
  signpath=true
  policy=test-signing
elif [[ "${PUBLISH:-false}" == true ]]; then
  case "${PROVIDER:-}" in
    '') echo '::notice::未设置 WINDOWS_SIGNING_PROVIDER，保留未签名发布。' ;;
    signpath)
      require_signpath
      [[ -n "${POLICY:-}" ]] || { echo '::error::缺少 vars.SIGNPATH_SIGNING_POLICY_SLUG'; exit 1; }
      if [[ "$GITHUB_REF_NAME" == main || "$GITHUB_REF_NAME" == develop ]]; then
        signpath=true
        policy="$POLICY"
      fi ;;
    simplysign)
      [[ "${GITHUB_REF_TYPE:-}" == branch && ( "$GITHUB_REF_NAME" == main || "$GITHUB_REF_NAME" == develop ) ]] || { echo '::error::SimplySign 发布只允许 main 或 develop 分支'; exit 1; }
      [[ "${GITHUB_REPOSITORY:-}" == metasequoiaime/msime ]] || { echo '::error::SimplySign runner 仅服务本仓库'; exit 1; }
      [[ "${SSIGN_SHA256:-}" =~ ^[[:xdigit:]]{64}$ ]] || { echo '::error::缺少有效的 WINDOWS_SSIGN_SHA256'; exit 1; }
      [[ "${CERTIFICATE_THUMBPRINT:-}" =~ ^[[:xdigit:]]{40}$ ]] || { echo '::error::缺少有效的 WINDOWS_SIGNING_CERTIFICATE_THUMBPRINT'; exit 1; }
      simplysign=true ;;
    *) echo '::error::未知的 WINDOWS_SIGNING_PROVIDER'; exit 1 ;;
  esac
fi
printf 'signpath=%s\nsimplysign=%s\npolicy=%s\n' "$signpath" "$simplysign" "$policy" >> "$GITHUB_OUTPUT"
