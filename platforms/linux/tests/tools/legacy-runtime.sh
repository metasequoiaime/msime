#!/usr/bin/env bash
# legacy 包（#6311）的运行时验收，由 package-legacy-container.sh 第 2 步在装好包的 debian:buster 容器里调用。ctest 只覆盖单元测试和合约，证明的是能链接、逻辑对；这里让已安装的 /usr/bin/msime-linux-ibus 在 buster 自带的 ibus-daemon 1.5.19 下真正打字，并在 Python 3.7 上跑随包脚本的合约测试。
#
# 参数：<已校验的词库目录> <构建树>。词库由第 1 步按 resources/desktop-dictionary.lock.json 取回，不随包安装；构建树里有 ibus-engine-smoke。
# 调用方先做完只装了包与其依赖时的检查（apt 安装、ldd、ctest），这里再装测试用的工具：dbus、Xvfb、xdotool、PyGObject 与 GTK 3 的 IBus 输入法模块，与 bookworm 验收镜像（tests/tools/Dockerfile）里的同一组。它们不进包的 Depends。
set -euo pipefail
[[ $# == 2 && -d $1 && -x $2/ibus-engine-smoke ]] || {
  echo "usage: legacy-runtime.sh <verified-resources> <build-tree>" >&2
  exit 2
}
resources=$1
build_tree=$2
export MSIME_ISOLATED_LINUX_TEST=1

DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
  dbus xvfb xauth xdotool x11-xkb-utils python3-gi gir1.2-ibus-1.0 gir1.2-gtk-3.0 ibus-gtk3 >/dev/null
echo "ibus-daemon $(dpkg-query -W -f '${Version}' ibus), $(python3 --version)"

# 随包 Python 脚本的合约测试，与 in-container.sh 同一份清单，跳过两项：
# - panel_keymap.py 核对的是 Tauri 设置窗口的按键表（apps/desktop），要 rustc 和内核头文件；legacy 包不带设置窗口。
# - doubao_auth.py 测的是 websockets 15 的豆包传输，靠 importlib.metadata（Python 3.8 起才有）伪造版本，legacy 包上豆包识别本来不可用（README 已写明）。改为确认 3.7 上依赖检查报告不满足，而不是在导入时崩溃。
# 测试本身用到 Python 3.8 才有的 mock 调用记录属性 .args 与 .kwargs，这里在测试进程里补上同样的只读属性再运行测试文件。只改测试进程里的 unittest.mock，不碰被测的随包脚本，它们照样在 3.7 上原样运行。
run_on_python37='
import os, runpy, sys
from unittest import mock
if not hasattr(mock._Call, "args"):
    mock._Call.args = property(lambda call: tuple.__getitem__(call, -2))
    mock._Call.kwargs = property(lambda call: tuple.__getitem__(call, -1))
sys.argv = sys.argv[1:]
sys.path[0] = os.path.dirname(os.path.abspath(sys.argv[0]))
runpy.run_path(sys.argv[0], run_name="__main__")
'
mapfile -t contracts < platforms/linux/tests/tools/python-contracts.list
for contract in "${contracts[@]}"; do
  case "$contract" in */panel_keymap.py|*/doubao_auth.py) continue ;; esac
  python3 -c "$run_on_python37" "$contract"
done
python3 -c '
import sys
sys.path.insert(0, "/usr/bin")
from msime_voice_doubao import WEBSOCKETS_REQUIRED, websocket_dependency
try:
    websocket_dependency()
except RuntimeError as error:
    assert str(error) == WEBSOCKETS_REQUIRED, error
else:
    raise AssertionError("Doubao transport reported usable on Python 3.7")
print("Doubao reports its dependency unmet on Python 3.7")
'

# 引擎在进程内对 IBus 1.5.19 的库跑候选翻页与选词，是 ctest 在有词库时登记的那一项（ibus-page-number-visibility）。不带参数的完整流程目前在 bookworm 与 IBus 1.5.27 上同样失败（英文模式录音时没有流式预编辑），与 IBus 版本无关，这里不跑。
"$build_tree/ibus-engine-smoke" "$resources" --page-number

# 用已安装的 msime-linux-prepare 准备状态目录，与用户运行 msime-linux-setup 时走的是同一个程序。
fixture=$(mktemp -d /tmp/msime-legacy-runtime.XXXXXX)
trap 'rm -rf "$fixture"' EXIT
msime-linux-prepare "$resources" "$fixture/state" >/dev/null
options=$fixture/state/runtime-options.json
global_mode_options=$fixture/global-runtime-options.json
# 与 in-container.sh 相同：不带图形客户端的 daemon_smoke.py 测的是全局中英文状态，要一份 ime_mode_scope 为 global 的配置。
python3 - "$options" "$global_mode_options" <<'PYTHON'
import json
import os
import sys
from pathlib import Path

value = json.loads(Path(sys.argv[1]).read_text())
assert value["preferences"]["default_ime_mode"] == "chinese", value["preferences"]["default_ime_mode"]
value["preferences"]["ime_mode_scope"] = "global"
value.pop("preferences_directory", None)
with os.fdopen(os.open(sys.argv[2], os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600), "w") as output:
    json.dump(value, output)
PYTHON

# 真实的 ibus-daemon 1.5.19：经已安装的启动器拉起宿主，合成输入上下文打字、切换输入源、宿主崩溃后由监护进程重启、ibus exit 收尾。
dbus-run-session -- bash platforms/linux/tests/runtime/daemon_smoke.sh /usr/bin/msime-linux-ibus "$global_mode_options"
# GTK 3 输入法模块经 IBus 1.5.19 把候选、编辑、焦点与密码框行为送到真实的文本框。
GTK_IM_MODULE=ibus XMODIFIERS=@im=ibus NO_AT_BRIDGE=1 xvfb-run -a dbus-run-session -- \
  bash platforms/linux/tests/runtime/daemon_smoke.sh /usr/bin/msime-linux-ibus "$options" platforms/linux/tests/runtime/gtk_smoke.py
echo "legacy runtime acceptance passed on IBus $(dpkg-query -W -f '${Version}' ibus)"
