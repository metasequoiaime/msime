#!/usr/bin/env bash
# 发版的两步（规则见 AGENTS.md 的「发版」）：先向 main 开只改版本号的 PR，合进 main 之后在那个提交上推发版 tag，tag 触发对应的 .github/workflows/release-<平台>.yml。
#
# 用法：
#   scripts/release-pr.sh [--dry-run] <平台> <版本号> [<平台> <版本号> ...]
#       从 origin/main 生成一个只改版本号的提交，推到 chore/<平台>-<版本号>-version 分支，向 main 开 PR。PR 和别的 PR 一样 squash 合并。
#   scripts/release-pr.sh --tag [--dry-run] [--pr <编号> | --ref <提交>] [--beta <N|bare>] <平台> ...
#       版本号 PR 合进 main 之后：在 --pr 合并出的提交（或 --ref，缺省 origin/main）上给每个平台打 <前缀>X.Y.Z 并推送；--beta N 打测试版 <前缀>X.Y.Z-beta.N，--beta bare 打不带编号的 <前缀>X.Y.Z-beta。X.Y.Z 取那个提交里的版本号，提交必须是 origin/main 的祖先。推上去的 tag 触发发布工作流，工作流会再核对一遍。
#       每个 tag 单独 git push 一次：一次推送超过三个 tag 时 GitHub 不为它们产生 push 事件，发布工作流一个都不会启动，也不报错。手动推 tag 同样不要用 git push --tags。
#       已经装在用户手上的 Windows、Linux、HarmonyOS 客户端（update_check.rs 改成认 -beta.N 之前的构建）只认不带编号的 -beta：那个平台第一个认 -beta.N 的正式版发出去以前，测试版用 --beta bare。Android 的预览版通道只比 X.Y.Z（UpdateApi.pick），同一个 X.Y.Z 上的第二个测试版不会提供给装了第一个的人，所以 Android 每个 X.Y.Z 只打一个测试版 tag，再发测试版先改版本号。
#
# 平台：android harmony ios linux macos windows windows7（Win7 是独立的版本线，版本号在 platforms/windows/version-win7.txt，只有正式版）。
#
# 开 PR 时全程只用 git 底层命令（hash-object、update-index、commit-tree），不检出任何分支，也不动当前工作区，在哪个 worktree 里运行都一样。
set -euo pipefail

remote=origin
dry_run=0
mode=bump
pr=""
ref=""
beta=""
args=()

while [ $# -gt 0 ]; do
  case "$1" in
    --dry-run) dry_run=1; shift ;;
    --tag) mode=tag; shift ;;
    --pr) pr="${2:?--pr 需要一个 PR 编号}"; shift 2 ;;
    --ref) ref="${2:?--ref 需要一个提交}"; shift 2 ;;
    --beta) beta="${2:?--beta 需要一个编号}"; shift 2 ;;
    -h | --help) sed -n '2,14p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    -*) echo "未知参数：$1" >&2; exit 2 ;;
    *) args+=("$1"); shift ;;
  esac
done

cd "$(git rev-parse --show-toplevel)"

# 各平台记版本号的文件、tag 前缀和显示名。
version_file() {
  case "$1" in
    windows7) echo platforms/windows/version-win7.txt ;;
    android | harmony | ios | linux | macos | windows) echo "platforms/$1/version.txt" ;;
    *) return 1 ;;
  esac
}
display_name() {
  case "$1" in
    android) echo Android ;; harmony) echo HarmonyOS ;; ios) echo iOS ;; linux) echo Linux ;;
    macos) echo macOS ;; windows) echo Windows ;; windows7) echo "Windows 7" ;;
  esac
}
read_version() { git show "$1:$(version_file "$2")" 2>/dev/null | tr -d '[:space:]' || true; }
is_version() { [[ "$1" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; }
check_platform() { version_file "$1" >/dev/null || { echo "未知平台：$1（可选 android harmony ios linux macos windows windows7）" >&2; exit 2; }; }

# 只取 main；tag 一律问远端（git ls-remote），不动本地的 tag。
git fetch --quiet "$remote" main
main=$(git rev-parse "$remote/main")
# 远端某个前缀的 tag 指向的提交，每行「tag 提交」；附注 tag 取它指向的提交（ls-remote 的 ^{} 行）。
remote_tags() {
  git ls-remote --tags "$remote" "refs/tags/$1*" | awk '{ name = $2; sub("^refs/tags/", "", name); if (sub("\\^\\{\\}$", "", name)) peeled[name] = $1; else plain[name] = $1 } END { for (name in plain) print name, (name in peeled ? peeled[name] : plain[name]) }'
}

# 组织的 windows-signing runner 组只让白名单里的 workflow@ref 用签名机，而白名单只收已经存在的具体 ref，不认 windows-v* 这样的通配，也不能不写 ref（API 直接 400）。所以每推一个 windows-v tag，就把 release-windows.yml@refs/tags/<tag> 加进去：签名 job 要等编译 job 跑完才排队，推完再加来得及。名单只留 main 和最近 5 个 windows tag；已删除的 ref（例如 develop）必须去掉，否则 API 校验整份名单时会拒绝更新。需要 gh 有 admin:org 权限；加不上时 tag 已经推了，按提示手动加完，再用 gh run rerun --failed 重跑签名那一步。
signing_group_id=3
# RELEASE_PR_GH 只给测试用：scripts/test-release-pr-tag.py 换成一个记录调用的假 gh，不碰真的组织设置。
gh_cmd=${RELEASE_PR_GH:-gh}
allow_windows_signing() {
  local entry="metasequoiaime/msime/.github/workflows/release-windows.yml@refs/tags/$1" current body
  if ! current=$("$gh_cmd" api "orgs/metasequoiaime/actions/runner-groups/$signing_group_id" --jq '.selected_workflows' 2>/dev/null); then
    echo "读不到 windows-signing runner 组（gh 需要 admin:org）：手动把 $entry 加进组织设置 → Actions → Runner groups → windows-signing 的 workflow 白名单" >&2
    return 1
  fi
  body=$(jq -c --arg e "$entry" '
    def tagged: test("@refs/tags/windows-v");
    (map(select(endswith("@refs/heads/main")))) as $main
    | ((map(select(tagged)) - [$e]) + [$e]) as $tags
    | {restricted_to_workflows: true, selected_workflows: ($main + $tags[-5:])}' <<<"$current")
  if "$gh_cmd" api -X PATCH "orgs/metasequoiaime/actions/runner-groups/$signing_group_id" --input - >/dev/null <<<"$body"; then
    echo "已放行 Windows 签名：$entry"
  else
    echo "没能把 $entry 加进 windows-signing 的白名单：手动加上，否则签名 job 拿不到 runner" >&2
    return 1
  fi
}

if [ "$mode" = tag ]; then
  [ ${#args[@]} -gt 0 ] || { echo "用法：scripts/release-pr.sh --tag [--pr <编号> | --ref <提交>] [--beta <N|bare>] <平台> ..." >&2; exit 2; }
  [ -z "$pr" ] || [ -z "$ref" ] || { echo "--pr 和 --ref 只能给一个" >&2; exit 2; }
  [ -z "$beta" ] || [ "$beta" = bare ] || [[ "$beta" =~ ^[1-9][0-9]*$ ]] || { echo "--beta 的编号从 1 起，不带前导零，或者是 bare：$beta" >&2; exit 2; }
  if [ -n "$pr" ]; then
    command -v gh >/dev/null || { echo "--pr 需要 gh CLI" >&2; exit 1; }
    state=$(gh pr view "$pr" --json state --jq .state)
    [ "$state" = MERGED ] || { echo "PR #$pr 是 $state，还没合进 main" >&2; exit 1; }
    ref=$(gh pr view "$pr" --json mergeCommit --jq .mergeCommit.oid)
  fi
  sha=$(git rev-parse --verify "${ref:-$main}^{commit}")
  git merge-base --is-ancestor "$sha" "$main" || { echo "${sha:0:12} 不是 $remote/main 的祖先；只在合进 main 的提交上打发版 tag" >&2; exit 1; }
  tags=()
  for platform in "${args[@]}"; do
    check_platform "$platform"
    version=$(read_version "$sha" "$platform")
    is_version "$version" || { echo "${sha:0:12} 里 $(version_file "$platform") 不是 X.Y.Z：'$version'" >&2; exit 1; }
    stable="$platform-v$version"
    tag="$stable"
    if [ -n "$beta" ]; then
      [ "$platform" != windows7 ] || { echo "Windows 7 只有正式版，不打 -beta tag" >&2; exit 1; }
      # 包内版本号就是 X.Y.Z：正式版发出后再在同一个版本号上发测试版，测试版通道会把它排在正式版之后，所以先经 PR 把版本号加一。
      if [ -n "$(git ls-remote --tags "$remote" "refs/tags/$stable")" ]; then
        echo "$stable 已经发过正式版；测试版要用还没发过的版本号，先用 scripts/release-pr.sh $platform <下一个版本号> 改版本号" >&2
        exit 1
      fi
      if [ "$platform" = android ] && [ -n "$(git ls-remote --tags "$remote" "refs/tags/$stable-beta*")" ]; then
        echo "$stable 已经发过测试版；Android 的预览版通道只比 X.Y.Z，同一个版本号上的下一个测试版不会提供给已经装了上一个的人，先用 scripts/release-pr.sh android <下一个版本号> 改版本号" >&2
        exit 1
      fi
      if [ "$beta" = bare ]; then tag="$stable-beta"; else tag="$stable-beta.$beta"; fi
    fi
    if [ -n "$(git ls-remote --tags "$remote" "refs/tags/$tag")" ]; then
      echo "$tag 已经存在；重跑失败的发布用 gh workflow run release-$platform.yml --ref $tag" >&2
      exit 1
    fi
    tags+=("$tag")
    echo "$tag -> ${sha:0:12}"
  done
  if [ "$dry_run" = 1 ]; then
    echo "（dry run，未打 tag）"
    exit 0
  fi
  for tag in "${tags[@]}"; do
    platform=${tag%%-v*}
    git tag -a "$tag" -m "$(display_name "$platform") ${tag#"$platform"-v}" "$sha"
  done
  # 一个 tag 推一次：GitHub 对一次推上来超过三个的 tag 不产生 push 事件（scripts/test-release-pr-tag.py 守着这一条）。set -e 让某一个推送失败时停在那里，前面推上去的照常发布。
  for tag in "${tags[@]}"; do
    git push "$remote" "refs/tags/$tag"
    echo "已推送 $tag：gh run list --workflow release-${tag%%-v*}.yml --limit 1"
    case "$tag" in windows-v*) allow_windows_signing "$tag" ;; esac
  done
  exit 0
fi

# ---- 版本号 PR ----
[ ${#args[@]} -gt 0 ] && [ $((${#args[@]} % 2)) -eq 0 ] || { echo "用法：scripts/release-pr.sh [--dry-run] <平台> <版本号> [<平台> <版本号> ...]" >&2; exit 2; }
[ -z "$pr$ref$beta" ] || { echo "--pr、--ref、--beta 只用于 --tag" >&2; exit 2; }
command -v gh >/dev/null || { echo "需要 gh CLI" >&2; exit 1; }
command -v python3 >/dev/null || { echo "需要 python3 生成有界 PR 正文" >&2; exit 1; }

index=$(mktemp)
body_file=$(mktemp)
trap 'rm -f "$index" "$body_file"' EXIT
export GIT_INDEX_FILE="$index"
git read-tree "$main"

# 把 <路径> 换成标准输入的内容，文件模式沿用 main 上的。
stage() {
  local path=$1 mode blob
  mode=$(git ls-tree "$main" -- "$path" | awk '{print $1}')
  [ -n "$mode" ] || { echo "main 上没有 $path" >&2; exit 1; }
  blob=$(git hash-object -w --stdin)
  git update-index --cacheinfo "$mode,$blob,$path"
}
# 用 sed -E 表达式改 main 上的 <路径>；表达式没有改到任何东西时失败，免得发出去的包版本号对不上。
edit() {
  local path=$1 expression=$2 before after
  before=$(git show "$main:$path")
  after=$(printf '%s\n' "$before" | sed -E "$expression")
  [ "$after" != "$before" ] || { echo "$path 里没找到要改的版本号（$expression）" >&2; exit 1; }
  printf '%s\n' "$after" | stage "$path"
}

seen=" "
names=()
branch_parts=()
version_rows=""
first_platform=""
set -- "${args[@]}"
while [ $# -gt 0 ]; do
  platform=$1 version=$2
  shift 2
  check_platform "$platform"
  case "$seen" in *" $platform "*) echo "$platform 给了两次" >&2; exit 2 ;; esac
  seen+="$platform "
  is_version "$version" || { echo "$platform 的版本号必须是 X.Y.Z：$version" >&2; exit 2; }
  old=$(read_version "$main" "$platform")
  [ "$old" != "$version" ] || { echo "main 上 $platform 已经是 $version" >&2; exit 1; }
  if [ -n "$old" ] && [ "$(printf '%s\n%s\n' "$old" "$version" | sort -V | tail -1)" != "$version" ]; then
    echo "$platform 的新版本号 $version 比 main 上的 $old 旧" >&2
    exit 1
  fi
  printf '%s\n' "$version" | stage "$(version_file "$platform")"
  case "$platform" in
    # Sparkle 按 CFBundleVersion 判断新旧，build-number.txt 在新版本上从 1 起（CMake 从这两个文件生成包内版本号）。
    macos) printf '1\n' | stage platforms/macos/build-number.txt ;;
    ios)
      edit platforms/ios/project.yml "s/^(    MARKETING_VERSION: ).*$/\\1$version/"
      edit platforms/ios/MSIMEClient.xcodeproj/project.pbxproj "s/(MARKETING_VERSION = )[0-9.]+;/\\1$version;/g" ;;
    # versionCode 按 major*1e6 + minor*1e3 + patch，不递增系统拒绝覆盖安装。
    harmony)
      IFS=. read -r major minor patch <<< "$version"
      edit platforms/harmony/AppScope/app.json5 "s/(\"versionName\": \")[^\"]*\"/\\1$version\"/; s/(\"versionCode\": )[0-9]+/\\1$((major * 1000000 + minor * 1000 + patch))/" ;;
  esac
  names+=("$(display_name "$platform") $version")
  branch_parts+=("$platform-$version")
  version_rows+="| $platform | ${old:--} | $version |"$'\n'
  [ -n "$first_platform" ] || first_platform=$platform
done

if [ ${#names[@]} -eq 1 ]; then
  title="chore(release): set ${names[0]% *} version to ${names[0]##* }"
else
  joined=$(printf '%s, ' "${names[@]}")
  title="chore(release): set versions (${joined%, })"
fi
branch="chore/$(IFS=-; echo "${branch_parts[*]}")-version"
[ -z "$(git ls-remote --heads "$remote" "refs/heads/$branch")" ] || { echo "远端已经有 $branch，先处理它对应的 PR" >&2; exit 1; }

tree=$(git write-tree)
unset GIT_INDEX_FILE
head=$(git commit-tree "$tree" -p "$main" -m "$title")

# 本次发版带进来的提交：从这个平台上一个发版 tag（正式版或测试版，取 main 上最靠后的那个）到 main；还没有 tag 时从根提交算起。
since=""
since_depth=-1
while read -r _ commit; do
  if [ -z "$commit" ] || ! git cat-file -e "$commit^{commit}" 2>/dev/null || ! git merge-base --is-ancestor "$commit" "$main"; then continue; fi
  depth=$(git rev-list --count "$commit")
  if [ "$depth" -gt "$since_depth" ]; then since=$commit since_depth=$depth; fi
done < <(remote_tags "$first_platform-v")
[ -n "$since" ] || since=$(git rev-list --max-parents=0 "$main" | tail -1)
commit_count=$(git rev-list --first-parent --count "$since..$main")
platforms_list=$(printf '%s ' "${args[@]}" | awk '{for (i = 1; i <= NF; i += 2) printf "%s%s", (i > 1 ? " " : ""), $i}')

{
  echo "## 摘要"
  echo
  echo "只改版本号，基于 main \`${main:0:9}\`。"
  echo
  echo "## 版本号"
  echo
  echo "| 平台 | main | 本 PR |"
  echo "| --- | --- | --- |"
  printf '%s' "$version_rows"
  echo
  echo "## 发布"
  echo
  echo "合并后在合并出的提交上推发版 tag，tag 触发发布工作流：\`scripts/release-pr.sh --tag --pr <本 PR 编号> $platforms_list\`（测试版加 \`--beta <N>\`，旧客户端还只认不带编号的 -beta 时用 \`--beta bare\`，见脚本开头的说明）。"
  echo
} >"$body_file"

# 正文先按保守字节预算生成并校验，避免推送成功后才被 GitHub 的字符上限拒绝。
repo_url=$(gh repo view --json url --jq .url)
python3 scripts/release-pr-body.py --prefix-file "$body_file" --output "$body_file" \
  --since "$since" --head "$main" --commit-count "$commit_count" --repo-url "$repo_url"

echo "分支：$branch -> ${head:0:9}"
echo "标题：$title"
if [ "$dry_run" = 1 ]; then
  echo "--- 改动（dry run，未推送）---"
  git diff --stat "$main" "$head"
  echo "--- PR 正文 ---"
  cat "$body_file"
  exit 0
fi

git push "$remote" "$head:refs/heads/$branch"
gh pr create --base main --head "$branch" --title "$title" --body-file "$body_file"
echo "合并后推发版 tag：scripts/release-pr.sh --tag --pr <PR 编号> $platforms_list"
