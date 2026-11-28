#!/bin/bash
# 安装包的 postinstall 模板，platforms/macos/scripts/installer_package.py render 按版本把 @APP_BUNDLE@、@IME_BUNDLE@、@IME_BUNDLE_ID@、@IME_EXECUTABLE@ 换成设置应用的文件名、内嵌输入法的文件名、bundle id 和可执行文件名。Installer 以 root 运行它，$2 是安装位置（/Applications），$3 是目标卷。
#
# 安装包自己把输入法装好，只要 Installer 的那一次管理员授权（.agents/notes/implemented/bug-fix/2026-11-28-macos-pkg-installs-input-method.md）：
# 1. 以 root 把设置应用内嵌的输入法原子地装进 /Library/Input Methods：先 ditto 到同一目录里的隐藏暂存目录，改属主和权限、codesign 校验、核对 bundle id，再把旧的改名挪开、新的改名进来、删掉旧的，任何一步失败都不留半份 bundle。已装的一份构建号相同（修订也相同）或更新、而且校验通过时不动它，与设置应用的启动检查一样从不降级。登录用户的 ~/Library/Input Methods 里已经有一份时也不动系统目录：那份优先，由设置应用更新。
# 2. 系统目录里有可用的输入法时，以 root 装上更新助手和它的 launchd 任务，路径、权限和载入方式与设置应用的 SYSTEM_INSTALL_SCRIPT（apps/desktop/src-tauri/src/platform/macos/macos_input_source.rs）相同，scripts/test-macos-package-resources.py 核对两边一致。
# 3. 只在图形安装、有正常登录用户时，以那个用户的身份登记输入源（lsregister -f、--register-input-source），刚换过 bundle 时停掉他正在运行的旧输入法进程、刷新系统设置的输入源缓存，最后打开设置应用：它在首次启动时准备用户的词库和配置，没有这一步输入法建不起会话。输入法装好时设置应用不显示首次安装窗口；装不上时那个窗口就是兜底。
# 命令行、Homebrew 和 MDM 安装（installer 设了 COMMAND_LINE_INSTALL）只做前两步。任何情况都以 0 退出：输入法装不上不该让已经装好的设置应用被报成安装失败，原因写进 /var/log/install.log。
set -u

app_name='@APP_BUNDLE@'
ime_name='@IME_BUNDLE@'
ime_bundle_id='@IME_BUNDLE_ID@'
ime_executable='@IME_EXECUTABLE@'
system_input_methods='/Library/Input Methods'
updater_tool='/Library/PrivilegedHelperTools/app.msime.system-updater'
updater_job='/Library/LaunchDaemons/app.msime.system-updater.plist'
updater_inbox='/Library/Application Support/app.msime.system-updater/inbox'
updater_label='app.msime.system-updater'
lsregister='/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister'

log() {
  echo "postinstall: $*"
}

# bundle 的构建号（CFBundleVersion），去掉前导零；不是十进制整数时为空。
bundle_build() {
  local build
  build="$(/usr/bin/plutil -extract CFBundleVersion raw -o - "$1/Contents/Info.plist" 2>/dev/null || true)"
  case "$build" in
    "" | *[!0-9]*) return 0 ;;
  esac
  build="${build#"${build%%[!0]*}"}"
  echo "${build:-0}"
}

# 按数值比较两个构建号，输出 -1、0 或 1。按长度再按字典序比，不经过 shell 的 64 位整数。
compare_builds() {
  if [ "${#1}" -ne "${#2}" ]; then
    [ "${#1}" -gt "${#2}" ] && echo 1 || echo -1
  elif [ "$1" = "$2" ]; then
    echo 0
  elif [[ "$1" > "$2" ]]; then
    echo 1
  else
    echo -1
  fi
}

bundle_revision() {
  /usr/bin/plutil -extract MSIMEInputSourceRevision raw -o - "$1/Contents/Info.plist" 2>/dev/null || true
}

# 真实目录、bundle id 对得上、可执行文件在、签名封印完整，才算一份能用的输入法。
bundle_usable() {
  [ -d "$1" ] && [ ! -L "$1" ] || return 1
  [ -f "$1/Contents/MacOS/$ime_executable" ] && [ ! -L "$1/Contents/MacOS/$ime_executable" ] || return 1
  [ "$(/usr/bin/plutil -extract CFBundleIdentifier raw -o - "$1/Contents/Info.plist" 2>/dev/null || true)" = "$ime_bundle_id" ] || return 1
  /usr/bin/codesign --verify --deep --strict "$1" >/dev/null 2>&1
}

location="${2:-}"
app="${location%/}/$app_name"
source="$app/Contents/Resources/$ime_name"
target="$system_input_methods/$ime_name"
if [ -z "$location" ] || [ -L "$app" ] || [ ! -d "$app" ]; then
  log "$app is not an installed app bundle, nothing to do"
  exit 0
fi
if [ "${3:-/}" != "/" ]; then
  log "installing onto ${3}, not the startup disk; $app_name installs the input method when it is opened there"
  exit 0
fi

# 正常登录用户：图形会话里的人，不是登录窗口、首次设置助手或系统账户。
user="$(/usr/bin/stat -f %Su /dev/console 2>/dev/null || true)"
uid=""
home=""
case "$user" in
  "" | root | loginwindow | _mbsetupuser)
    log "no regular console user (${user:-none})"
    user=""
    ;;
  *)
    uid="$(/usr/bin/id -u "$user" 2>/dev/null || true)"
    case "$uid" in
      "" | *[!0-9]*)
        log "cannot resolve the uid of $user"
        user=""
        ;;
      *)
        if [ "$uid" -lt 501 ]; then
          log "console user $user is a system account (uid $uid)"
          user=""
        else
          home="$(/usr/bin/dscl . -read "/Users/$user" NFSHomeDirectory 2>/dev/null | /usr/bin/sed -n 's/^NFSHomeDirectory: //p' | /usr/bin/head -n 1)"
        fi
        ;;
    esac
    ;;
esac

as_user() {
  /bin/launchctl asuser "$uid" /usr/bin/sudo -u "$user" -H "$@"
}

# 1. 输入法。
installed_now=""
system_ready=""
user_copy=""
case "$home" in
  /*)
    if [ -e "$home/Library/Input Methods/$ime_name" ] || [ -L "$home/Library/Input Methods/$ime_name" ]; then
      user_copy="$home/Library/Input Methods/$ime_name"
    fi
    ;;
esac

install_input_method() {
  if [ -L "$source" ] || [ ! -d "$source" ]; then
    log "$source is not a bundle, the input method is not installed"
    return 1
  fi
  local wanted installed
  wanted="$(bundle_build "$source")"
  if [ -z "$wanted" ]; then
    log "cannot read the build of $source, the input method is not installed"
    return 1
  fi
  if [ -L "$system_input_methods" ]; then
    log "$system_input_methods is a symbolic link, the input method is not installed"
    return 1
  fi
  if [ -e "$target" ] || [ -L "$target" ]; then
    installed="$(bundle_build "$target")"
    # 与设置应用的 refresh_decision 一致：已装的更新时不降级；构建号相同时只有随附的带修订、且修订不同才换。已装的那份坏了（校验不过）就换。
    local order=""
    [ -z "$installed" ] || order="$(compare_builds "$installed" "$wanted")"
    if [ "$order" = 1 ] || { [ "$order" = 0 ] && { [ -z "$(bundle_revision "$source")" ] || [ "$(bundle_revision "$source")" = "$(bundle_revision "$target")" ]; }; }; then
      if bundle_usable "$target"; then
        log "$target (build $installed) is current for build $wanted, left as it is"
        return 0
      fi
      log "$target (build $installed) does not validate, replacing it"
    fi
  fi

  /bin/mkdir -p "$system_input_methods" || return 1
  # 暂存和备份放在同一目录里，改名才是原子的；名字以点开头、不以 .app 结尾，系统不会把它们当成输入法。上一次被中断留下的同名目录先清掉。
  /bin/rm -rf "$system_input_methods/.$ime_name.pkg-staging-"* "$system_input_methods/.$ime_name.pkg-previous-"*
  local staging="$system_input_methods/.$ime_name.pkg-staging-$$"
  local previous="$system_input_methods/.$ime_name.pkg-previous-$$"
  local staged="$staging/$ime_name"
  if ! /bin/mkdir -m 0700 "$staging"; then
    log "cannot create $staging"
    return 1
  fi
  # 权限不在签名封印里：目录 0755、文件 0644（带执行位的 0755），与设置应用的 make_readable_by_everyone 和更新助手一致，每个用户的输入法进程都读得了（#10329）。
  if ! /usr/bin/ditto "$source" "$staged" \
    || ! /usr/sbin/chown -R root:wheel "$staged" \
    || ! /usr/bin/find "$staged" -type d -exec /bin/chmod 0755 {} + \
    || ! /usr/bin/find "$staged" -type f -perm +111 -exec /bin/chmod 0755 {} + \
    || ! /usr/bin/find "$staged" -type f ! -perm +111 -exec /bin/chmod 0644 {} +; then
    log "cannot stage $ime_name in $system_input_methods"
    /bin/rm -rf "$staging"
    return 1
  fi
  if ! bundle_usable "$staged"; then
    log "the staged $ime_name does not validate (bundle id, executable or code signature), the input method is not installed"
    /bin/rm -rf "$staging"
    return 1
  fi
  if [ -e "$target" ] || [ -L "$target" ]; then
    if ! /bin/mv "$target" "$previous"; then
      log "cannot move the installed $target aside"
      /bin/rm -rf "$staging"
      return 1
    fi
  fi
  if [ -e "$target" ] || ! /bin/mv "$staged" "$target"; then
    log "cannot move the new $ime_name into place, restoring the previous one"
    if [ -e "$previous" ] || [ -L "$previous" ]; then
      /bin/mv "$previous" "$target"
    fi
    /bin/rm -rf "$staging"
    return 1
  fi
  /bin/rm -rf "$previous" "$staging"
  installed_now=1
  log "installed $ime_name (build $wanted) in $system_input_methods"
  return 0
}

if [ -n "$user_copy" ]; then
  log "$user_copy is installed for $user and takes priority; $system_input_methods is left alone and $app_name updates that copy"
elif install_input_method; then
  system_ready=1
fi

# 2. 更新助手。与 SYSTEM_INSTALL_SCRIPT 一样，装不上不算安装失败，只是以后的更新要再授权。
updater_source="$app/Contents/MacOS/msime-system-updater"
updater_job_source="$app/Contents/Resources/app.msime.system-updater.plist"
if [ -z "$system_ready" ]; then
  log "no usable input method in $system_input_methods, the system updater is not installed"
elif [ -L "$updater_source" ] || [ ! -f "$updater_source" ] || [ -L "$updater_job_source" ] || [ ! -f "$updater_job_source" ]; then
  log "$app_name carries no system updater, it is not installed"
elif /bin/mkdir -p "$(/usr/bin/dirname "$updater_tool")" "$updater_inbox" \
  && /bin/chmod 1777 "$updater_inbox" \
  && /usr/bin/install -o root -g wheel -m 755 "$updater_source" "$updater_tool" \
  && /usr/bin/install -o root -g wheel -m 644 "$updater_job_source" "$updater_job"; then
  /bin/launchctl bootout "system/$updater_label" >/dev/null 2>&1
  if /bin/launchctl bootstrap system "$updater_job" >/dev/null 2>&1 || { /bin/sleep 2; /bin/launchctl bootstrap system "$updater_job" >/dev/null 2>&1; }; then
    log "installed and loaded the system updater"
  else
    log "installed the system updater, but launchd did not load it"
  fi
else
  log "could not install the system updater"
fi

# 3. 登录用户这一侧。
if [ -n "${COMMAND_LINE_INSTALL:-}" ]; then
  log "command-line install: the input source is registered at the next login, and $app_name prepares the dictionary when it is first opened"
  exit 0
fi
if [ -z "$user" ]; then
  exit 0
fi

if [ -n "$system_ready" ]; then
  # 顺序与设置应用相同：先登记再停旧进程，反过来系统会在 LaunchServices 记录还指着旧副本时重新拉起输入法。
  if [ -x "$lsregister" ]; then
    as_user "$lsregister" -f "$target" >/dev/null 2>&1
  fi
  if as_user "$target/Contents/MacOS/$ime_executable" --register-input-source >/dev/null 2>&1; then
    log "registered the input source for $user"
  else
    log "the input source is not registered for $user yet; on a Mac where it was never installed it appears after the next login"
  fi
  if [ -n "$installed_now" ]; then
    pattern="$(printf '%s' "$target/Contents/MacOS/$ime_executable" | /usr/bin/sed 's/[][\.^$*+?(){}|]/\\&/g')"
    as_user /usr/bin/pkill -u "$uid" -f "$pattern" >/dev/null 2>&1
    # 系统设置「添加」对话框和 Control-空格 的切换器在各自的进程里缓存输入源列表，与设置应用的 refresh_system_input_source_lists 一样清掉，更新新增的模式不用重新登录就能添加。
    as_user /usr/bin/pkill -x 'System Settings' >/dev/null 2>&1
    as_user /usr/bin/pkill -f '/System/Library/ExtensionKit/Extensions/KeyboardSettings\.appex/Contents/MacOS/KeyboardSettings' >/dev/null 2>&1
    cache="$(as_user /usr/bin/getconf DARWIN_USER_CACHE_DIR 2>/dev/null || true)"
    case "$cache" in
      /*)
        cache="${cache%/}/com.apple.Keyboard-Settings.extension"
        if [ -d "$cache" ] && [ ! -L "$cache" ]; then
          as_user /usr/bin/find "$cache" -maxdepth 1 -type f -name 'com.apple.IntlDataCache.le*' -delete >/dev/null 2>&1
        fi
        ;;
    esac
    as_user /usr/bin/pkill -KILL -x CursorUIViewService >/dev/null 2>&1
    as_user /usr/bin/pkill -KILL -x TextInputSwitcher >/dev/null 2>&1
  fi
fi

# -n：不加时 LaunchServices 可能去激活输入法拉起的隐藏面板进程（与设置应用同一个 bundle id），面板不跑启动检查。-H：HOME 必须是登录用户的，设置应用在那里准备词库。open 启动应用后就返回，不等它退出。
if as_user /usr/bin/open -n "$app"; then
  log "opened $app for $user"
else
  log "could not open $app for $user"
fi
exit 0
