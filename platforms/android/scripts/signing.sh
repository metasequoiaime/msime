# Sourced by every script that signs an APK of app.msime.android (build-apk.sh, build-client-apk.sh, tests/device/build-editor.sh); sets the apksigner arguments in android_signing and a label in android_signed_with.
# 正式版、beta、本地开发构建一律用同一把发布密钥（别名 msime-release，证书 SHA-256 3a889e43…）签名：Android 只允许同证书覆盖安装，任何一个渠道换了证书，装过它的设备就只能卸载重装、丢掉本地词库和设置。0.2.0 时本地构建退回各机器自己的 debug.keystore（本机和 Mac Studio 各一把），装过本地包的设备装正式包时提示要重装。
# 取密钥的顺序：MSIME_ANDROID_RELEASE_KEYSTORE 与 MSIME_ANDROID_RELEASE_KEYSTORE_PASSWORD（release-android.yml 从仓库 secrets 解出）；否则 ~/.android/msime-release.p12，口令取环境变量或 macOS 钥匙串的 msime-android-release-keystore 条目。有密钥文件却拿不到口令时直接失败，不悄悄换成别的证书。两处都没有（贡献者的机器、fork 的 CI）才退回 dev-keystore.sh 的开发密钥，并提醒这样的包不能覆盖安装正式包。
android_release_keystore=${MSIME_ANDROID_RELEASE_KEYSTORE:-$HOME/.android/msime-release.p12}
if [[ -n "${MSIME_ANDROID_RELEASE_KEYSTORE:-}" || -f "$android_release_keystore" ]]; then
  [[ -f "$android_release_keystore" ]] || { echo "MSIME_ANDROID_RELEASE_KEYSTORE names $android_release_keystore, which does not exist" >&2; exit 1; }
  if [[ -z "${MSIME_ANDROID_RELEASE_KEYSTORE_PASSWORD:-}" ]] && command -v security >/dev/null; then
    MSIME_ANDROID_RELEASE_KEYSTORE_PASSWORD=$(security find-generic-password -s msime-android-release-keystore -w 2>/dev/null || true)
  fi
  [[ -n "${MSIME_ANDROID_RELEASE_KEYSTORE_PASSWORD:-}" ]] || { echo "$android_release_keystore needs its password in MSIME_ANDROID_RELEASE_KEYSTORE_PASSWORD or in the login keychain as msime-android-release-keystore (an SSH session cannot read the keychain); refusing to sign with a different key" >&2; exit 1; }
  export MSIME_ANDROID_RELEASE_KEYSTORE_PASSWORD
  android_signing=(--ks "$android_release_keystore" --ks-key-alias msime-release
    --ks-pass env:MSIME_ANDROID_RELEASE_KEYSTORE_PASSWORD --key-pass env:MSIME_ANDROID_RELEASE_KEYSTORE_PASSWORD)
  android_signed_with="release key"
else
  android_dev_keystore=$(bash "$repo_root/platforms/android/scripts/dev-keystore.sh")
  android_signing=(--ks "$android_dev_keystore" --ks-key-alias androiddebugkey --ks-pass pass:android --key-pass pass:android)
  android_signed_with="development key"
  echo "warning: no release key ($android_release_keystore); signing with the development key $android_dev_keystore, so this APK cannot update an installed release build and a release build cannot update it" >&2
fi
