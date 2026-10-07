#!/usr/bin/env bash
# 打印没有发布密钥的机器上 Android 开发构建签名用的 keystore 路径，首次运行时创建它；有发布密钥时 scripts/signing.sh 不会用到这里。这就是 Android SDK 自带的 debug keystore（别名 `androiddebugkey`，密码 `android`），因此所有 worktree 以及 Tauri 和 Android Studio 的 debug 构建都用同一把密钥签名，在一个 worktree 里构建的包可以直接覆盖安装另一个 worktree 构建的包。以前每个 worktree 在 `target/` 下各自保存一把密钥，结果每个新 worktree 的第一次构建都无法更新已安装的包，只能连同数据一起卸载。
set -euo pipefail
keystore="$HOME/.android/debug.keystore"
if [[ ! -f "$keystore" ]]; then
  mkdir -p "$(dirname "$keystore")"
  keytool -genkeypair -keystore "$keystore" -storepass android -keypass android \
    -alias androiddebugkey -dname "CN=Android Debug,O=Android,C=US" -keyalg RSA -keysize 2048 -validity 10000 >&2
fi
printf '%s\n' "$keystore"
