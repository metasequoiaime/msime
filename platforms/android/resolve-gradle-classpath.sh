#!/usr/bin/env bash
# 在真正构建之前先让 Gradle 解析一遍构建脚本的 classpath（AGP 8.13.2 及其传递依赖，大多只在 Maven Central 上），只在下载出错时整轮重来。
# 2026-11-17 起 GitHub 托管 runner 上的 Maven Central 时常回 429 Too Many Requests：同一分钟起跑的兄弟作业都能下载，只有个别 runner 被限流。Gradle 遇到 429 只重试两次（等 1、2 秒），随后在这次构建里停用整个仓库，于是 Release Android 0.8.3 的五笔版（run 44252182195）和三次 CI 设备作业都在配置阶段失败。发版作业里这一步排在解出签名密钥之前，重来不会延长密钥落盘的时间。
# 只认下载类错误（408、429、5xx、超时、连接被重置、域名解析失败）才重试；脚本写错、依赖不存在（404）或被拒（403）原样立即退出。
# 用法：bash platforms/android/resolve-gradle-classpath.sh
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."
android_sdk=${ANDROID_SDK_ROOT:-${ANDROID_HOME:?Set ANDROID_SDK_ROOT or ANDROID_HOME}}
project=${MSIME_GRADLE_PROJECT_DIR:-platforms/android/gradle-app}
# 每次重来前等待的秒数，一共三次尝试。Gradle 自己的退避由 workflow 的 GRADLE_OPTS 放宽（见 release-android.yml），这里接住比那更长的限流。
delays=(60 120)
attempts=$((${#delays[@]} + 1))
# 依赖解析的失败写成「Received status code 429 from server」；wrapper 下载 Gradle 发行包失败时是 Java 的「Server returned HTTP response code: 503」或套接字异常。
transient='Received status code (408|429|5[0-9][0-9]) from server|Server returned HTTP response code: (408|429|5[0-9][0-9])|(Read|[Cc]onnect(ion)?) timed out|SocketTimeoutException|Connection reset|UnknownHostException|Temporary failure in name resolution'
log=$(mktemp)
trap 'rm -f "$log"' EXIT
for ((attempt = 1; attempt <= attempts; attempt++)); do
  echo "Resolve the Gradle build classpath (attempt $attempt/$attempts)"
  status=0
  # help 不产出什么，但要先配置根工程，也就要下载并加载构建脚本 classpath 上的全部插件依赖。
  ANDROID_HOME="$android_sdk" "$project/gradlew" --project-dir "$project" --console=plain help 2>&1 | tee "$log" || status=$?
  if ((status == 0)); then exit 0; fi
  if ! grep -Eq "$transient" "$log"; then
    echo "Gradle failed with status $status, and not on a download; not retrying" >&2
    exit "$status"
  fi
  if ((attempt == attempts)); then break; fi
  delay=${delays[attempt - 1]}
  echo "Gradle could not download its build classpath: $(grep -Eom1 "$transient" "$log"); retrying in ${delay}s" >&2
  sleep "$delay"
done
echo "Gradle could not download its build classpath after $attempts attempts" >&2
exit 1
