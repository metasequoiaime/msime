#!/bin/bash
# 安装包的 postinstall 模板，platforms/macos/scripts/installer_package.py render 按版本把 @APP_BUNDLE@ 换成设置应用的文件名。Installer 以 root 运行它，参数 $2 是安装位置（/Applications）。
#
# 只做一件事：图形安装时以登录用户的身份打开刚装好的设置应用，让用户直接看到它的安装窗口（点「立即安装」把输入法装进自己的 ~/Library/Input Methods）。不写任何输入法目录、不注册输入源、不结束任何进程、不改属主：这些都由设置应用以用户身份完成，或者交给分发文件的 must-close。
# 命令行、Homebrew 和 MDM 安装（installer 设了 COMMAND_LINE_INSTALL）、登录窗口或系统账户在前台时什么都不做。任何情况都以 0 退出：打不开应用不该让已经装好的应用被报成安装失败。
set -u

app_name='@APP_BUNDLE@'

if [ -n "${COMMAND_LINE_INSTALL:-}" ]; then
  echo "postinstall: command-line install, not opening $app_name"
  exit 0
fi

user="$(/usr/bin/stat -f %Su /dev/console 2>/dev/null || true)"
case "$user" in
  "" | root | loginwindow | _mbsetupuser)
    echo "postinstall: no regular console user (${user:-none}), not opening $app_name"
    exit 0
    ;;
esac
uid="$(/usr/bin/id -u "$user" 2>/dev/null || true)"
case "$uid" in
  "" | *[!0-9]*)
    echo "postinstall: cannot resolve the uid of $user, not opening $app_name"
    exit 0
    ;;
esac
if [ "$uid" -lt 501 ]; then
  echo "postinstall: console user $user is a system account (uid $uid), not opening $app_name"
  exit 0
fi

location="${2:-}"
app="${location%/}/$app_name"
if [ -z "$location" ] || [ -L "$app" ] || [ ! -d "$app" ]; then
  echo "postinstall: $app is not an installed app bundle, not opening it"
  exit 0
fi

# -n：不加时 LaunchServices 可能去激活输入法拉起的隐藏面板进程（与设置应用同一个 bundle id），面板不跑安装检查。-H：HOME 必须是登录用户的，否则设置应用会把输入法装进 /var/root。open 启动应用后就返回，不等它退出。
if /bin/launchctl asuser "$uid" /usr/bin/sudo -u "$user" -H /usr/bin/open -n "$app"; then
  echo "postinstall: opened $app for $user"
else
  echo "postinstall: could not open $app for $user"
fi
exit 0
