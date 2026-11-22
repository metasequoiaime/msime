#!/usr/bin/env bash
# 编译设备测试包的 Java：`tests/device/` 下的每个 `.java` 都是入口，它们用到的应用类由 javac 按引用自动找来，不再手写清单。
# 应用源文件按功能分在子目录里，`package` 却大多是 `app.msime.android`，目录和包对不上，javac 的 -sourcepath 按包路径找不到它们。这里先在临时目录里按包路径建一份符号链接视图交给 -sourcepath；-implicit:class 只编译入口直接或间接引用到的类，测试包不会带上整个应用。
# 视图只收 `.java`：设备测试一旦引用到 Kotlin 类、或只有 Gradle 才能编译的类（引用 AndroidX 等），照样在这一步编译失败。
# build-editor.sh 打包和 check-host.sh 的编译检查共用本脚本，两边编出的类相同。
set -euo pipefail
android_jar=${1:?usage: compile-sources.sh <android.jar> <class output directory>}
output=${2:?class output directory required}
repo_root=$(cd "$(dirname "$0")/../../../.." && pwd)
android="$repo_root/platforms/android"
mkdir -p "$repo_root/target/android" "$output"
view=$(mktemp -d "$repo_root/target/android/device-sources.XXXXXX")
# 视图里只有符号链接，rm 不会跟进链接删到仓库里的源文件。
trap 'rm -rf "$view" "$view.map" "$view.err"' EXIT

# 视图的来源：应用源码、JVM 冒烟（设备测试复用其中的合成用例）和 OCR 合成夹具。每个文件按自己 `package` 行的包路径放进视图；没有 `package` 行的 JVM 冒烟在默认包里，有包的代码引用不到它们，不进视图。
packages=$(find "$android/java" "$android/tests" "$android/gradle-app/ocr-fixture" -name '*.java' -not -path "$android/tests/device/*" \
  -exec awk '/^package [A-Za-z0-9_.]+;/ { name = $2; sub(/;.*/, "", name); print FILENAME "\t" name; nextfile }' {} + | LC_ALL=C sort)
if [[ -z $packages ]]; then
  echo "No Android sources found for the device-suite source view" >&2
  exit 1
fi
while IFS=$'\t' read -r source package; do
  target="$view/${package//.//}/$(basename "$source")"
  # 同一个包里两个同名文件，javac 只会看到其中一个；直接失败，免得编进去的是哪一份取决于目录顺序。
  if [[ -L $target ]]; then
    echo "Two sources declare the same class for the device suite: ${source#"$repo_root/"} and $(readlink "$target" | sed "s|^$repo_root/||")" >&2
    exit 1
  fi
  mkdir -p "$(dirname "$target")"
  ln -s "$source" "$target"
  printf '%s\t%s\n' "$target" "${source#"$repo_root/"}" >> "$view.map"
done <<< "$packages"

roots=()
while IFS= read -r source; do
  roots+=("$source")
done < <(find "$android/tests/device" -name '*.java' -print | LC_ALL=C sort)
if [[ ${#roots[@]} -eq 0 ]]; then
  echo "No Android device-suite sources found under platforms/android/tests/device" >&2
  exit 1
fi

# 报错里的视图路径换回仓库路径，视图在退出时就删掉了。
if ! javac --release 17 -Xlint:all -Werror -implicit:class -cp "$android_jar" -sourcepath "$view" -d "$output" "${roots[@]}" 2> "$view.err"; then
  awk -F '\t' 'NR == FNR { repo[$1] = $2; next }
    { for (path in repo) { at = index($0, path); if (at) { $0 = substr($0, 1, at - 1) repo[path] substr($0, at + length(path)); break } } print }' \
    "$view.map" "$view.err" >&2
  exit 1
fi
cat "$view.err" >&2
echo "Compiled ${#roots[@]} Android device-suite sources and the app classes they reach"
