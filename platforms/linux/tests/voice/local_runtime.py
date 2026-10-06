#!/usr/bin/env python3
"""msime-voice-local 能打开 sherpa-onnx 运行库：经语音服务自己的 HelperPool 启动助手，要求它的 hello 报告运行库可用。缺依赖、架构不对或缺符号在这里失败，而不是等到用户第一次听写。

不给运行库时助手按自己所在的目录查找，用来检查装好的布局。

Usage: local_runtime.py <msime-voice-local> [<libsherpa-onnx-c-api.so>]
"""
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "scripts"))
import msime_voice_local as local  # noqa: E402


def main():
    helper, *runtime = sys.argv[1:]
    try:
        local.HelperPool(helper, *runtime).acquire().kill()
    except local.LocalUnavailable as error:
        sys.exit("the runtime did not load: %s" % error)
    print("sherpa-onnx runtime available")


if __name__ == "__main__":
    main()
