#!/usr/bin/env bash
# 一键从 origin/develop 切 release/<版本号> 分支并向 main 开发版 PR（规则见 AGENTS.md 的「发版」和 .github/workflows/branch-guard.yml）。
#
# 用法：scripts/release-pr.sh [--version <版本号>] [--dry-run]
#
# 全程只用 git 底层命令（merge-tree、commit-tree），不检出任何分支，也不动当前工作区，在哪个 worktree 里运行都一样。
#
# main 落后 develop 时分支需要把 main 记为祖先，否则合入时会冲突。按以下顺序处理：
#
# - main 已经是 develop 的祖先：分支直接指向 develop。
# - develop 与 main 能干净合并：用合并结果生成合并提交。
# - main 的 tree 与 develop 历史上某个提交的 tree 完全相同（典型情况是上一次发版 PR 被 squash 合并）：main 没有 develop 缺少的内容，生成一个保留 develop tree 的合并提交，等价于 `git merge -s ours origin/main`。
# - 其余情况说明 main 上有 develop 没有的改动（例如直接修在 main 上的 hotfix），脚本列出冲突文件后退出，需要人工处理。
set -euo pipefail

remote=origin
version=""
dry_run=0

while [ $# -gt 0 ]; do
  case "$1" in
    --version) version="${2:?--version 需要一个值}"; shift 2 ;;
    --dry-run) dry_run=1; shift ;;
    -h | --help) sed -n '2,13p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "未知参数：$1" >&2; exit 2 ;;
  esac
done

cd "$(git rev-parse --show-toplevel)"
command -v gh >/dev/null || { echo "需要 gh CLI" >&2; exit 1; }

git fetch --quiet "$remote" develop main
develop=$(git rev-parse "$remote/develop")
main=$(git rev-parse "$remote/main")
develop_tree=$(git rev-parse "$develop^{tree}")
main_tree=$(git rev-parse "$main^{tree}")

if [ "$develop_tree" = "$main_tree" ]; then
  echo "main 的内容已经与 develop 相同，无需发版。"
  exit 0
fi

existing=$(gh pr list --base main --state open --json headRefName,url --jq '.[] | select(.headRefName | startswith("release/")) | .url')
if [ -n "$existing" ]; then
  echo "已经有打开的发版 PR，先处理它：" >&2
  echo "$existing" >&2
  exit 1
fi

platforms=(android harmony ios linux macos windows)
read_version() { git show "$1:platforms/$2/version.txt" 2>/dev/null | tr -d '[:space:]' || true; }

version_rows=""
version_changed=0
for p in "${platforms[@]}"; do
  old=$(read_version "$main" "$p")
  new=$(read_version "$develop" "$p")
  [ "$old" != "$new" ] && version_changed=1
  version_rows+="| $p | ${old:--} | ${new:--} |"$'\n'
done

# 分支名沿用以往的 release/<macOS 版本号>；同名分支已存在（例如版本号不变的同步发版）时追加日期和序号。
[ -n "$version" ] || version=$(read_version "$develop" macos)
[ -n "$version" ] || { echo "读不到 platforms/macos/version.txt，请用 --version 指定" >&2; exit 1; }
remote_has() { [ -n "$(git ls-remote --heads "$remote" "refs/heads/$1")" ]; }
branch="release/$version"
if remote_has "$branch"; then
  branch="release/$version-$(date +%Y%m%d)"
  n=2
  while remote_has "$branch"; do
    branch="release/$version-$(date +%Y%m%d)-$n"
    n=$((n + 1))
  done
fi

# 本次发版的起点：main 的内容对应的那个 develop 提交，找不到时退回合并基点。用来统计和列出 main 新增的提交。
since=$(git log --first-parent --format='%H %T' "$develop" | awk -v t="$main_tree" '!found && $2 == t { print $1; found = 1 }')
merge_note=""
merge_clean=0
merged_tree=$(git merge-tree --write-tree "$develop" "$main" 2>/dev/null) && merge_clean=1
merged_tree=${merged_tree%%$'\n'*}
if git merge-base --is-ancestor "$main" "$develop"; then
  head=$develop
  merge_note="main 已经是 develop 的祖先，分支直接指向 develop \`${develop:0:9}\`。"
elif [ "$merge_clean" = 1 ]; then
  head=$(git commit-tree "$merged_tree" -p "$develop" -p "$main" -m "chore(release): 将 main 合入 $branch")
  if [ "$merged_tree" = "$develop_tree" ]; then
    merge_note="分支末尾是 develop \`${develop:0:9}\` 与 main 的合并提交，合并后的内容与 develop 完全一致。"
  else
    merge_note="分支末尾是 develop \`${develop:0:9}\` 与 main 的合并提交。**main 上有 develop 没有的改动，合并结果与 develop 不同**，合入 main 后记得把这些改动同步回 develop。"
  fi
elif [ -n "$since" ]; then
  head=$(git commit-tree "$develop_tree" -p "$develop" -p "$main" -m "chore(release): 将 main 合入 ${branch}，保留 develop 的内容")
  merge_note="main 的内容与 develop 的 \`${since:0:9}\` 完全相同（上一次发版被 squash 合并，main 不在 develop 的祖先里），分支末尾用等价于 \`git merge -s ours $remote/main\` 的合并提交把 main 记为祖先，内容与 develop \`${develop:0:9}\` 逐字节一致，不会丢掉 main 上的任何内容。"
else
  echo "main 上有 develop 没有的改动，无法自动合并。冲突文件：" >&2
  git merge-tree --write-tree --name-only "$develop" "$main" | sed -n '2,/^$/p' >&2 || true
  echo "请先把 main 的改动同步回 develop，再重新运行。" >&2
  exit 1
fi

[ -n "$since" ] || since=$(git merge-base "$develop" "$main")
commit_count=$(git rev-list --first-parent --count "$since..$develop")

if [ "$version_changed" = 1 ]; then
  title="chore(release): 发布 $version"
else
  title="chore(release): 同步 develop 到 main（版本号不变）"
fi

body_file=$(mktemp)
trap 'rm -f "$body_file"' EXIT
{
  echo "## 摘要"
  echo
  echo "从 develop \`${develop:0:9}\` 切出 \`$branch\`，把 develop 上 ${commit_count} 个提交带到 main。"
  echo
  echo "**请用 merge commit 合并，不要 squash。** squash 会让 main 上出现 develop 没有的提交，下一次发版就会在双方各自改过的文件上冲突。"
  echo
  echo "## 版本号"
  echo
  echo "| 平台 | main | develop |"
  echo "| --- | --- | --- |"
  printf '%s' "$version_rows"
  echo
  echo "## 合并方式"
  echo
  echo "$merge_note"
  echo
  echo "## 新增提交"
  echo
  echo "<details><summary>${commit_count} 个提交</summary>"
  echo
  git log --first-parent --format='- %s' "$since..$develop"
  echo
  echo "</details>"
} >"$body_file"

echo "分支：$branch -> ${head:0:9}"
echo "标题：$title"
if [ "$dry_run" = 1 ]; then
  echo "--- PR 正文（dry run，未推送）---"
  cat "$body_file"
  exit 0
fi

git push "$remote" "$head:refs/heads/$branch"
gh pr create --base main --head "$branch" --title "$title" --body-file "$body_file"
