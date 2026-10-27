"""产品 Python 进程的独立采集；异常文本、局部变量和私有路径不进入记录。"""

import atexit
import ctypes
import os
from pathlib import Path
import re
import sys


_capture = None
_preferences = ""
_options = ""
_source_names = frozenset((
    "msime-linux-online-provider", "msime-linux-voice-provider", "msime-linux-clipboard-monitor",
    "msime-linux-setup", "msime-linux-provider-session",
    "msime_provider_runtime.py", "msime_voice_doubao.py", "msime_voice_local.py",
    "msime_voice_polish_prompts.py", "msime_voice_provider_catalog.py", "host_api.py",
))


def _frames(traceback):
    frames = []
    root = Path(__file__).resolve().parent
    while traceback is not None:
        code = traceback.tb_frame.f_code
        source = Path(code.co_filename)
        if (source.name in _source_names and source.resolve().parent == root and
                re.fullmatch(r"[A-Za-z0-9_<>-]{1,128}", code.co_name)):
            frames.append("%s!%s:%d" % (source.name, code.co_name, traceback.tb_lineno))
        traceback = traceback.tb_next
    # Python 回溯从调用方到抛出方；与原生栈一致，先放真正抛出的产品帧。
    return ("\n".join(reversed(frames[-32:])) + "\n").encode("ascii")


class _Capture:
    def __init__(self, library, handle):
        self.library = library
        self.handle = handle
        self.previous_hook = sys.excepthook
        self.hook = self.exception
        sys.excepthook = self.hook

    def exception(self, kind, value, traceback):
        try:
            if kind not in (KeyboardInterrupt, SystemExit):
                self.library.msime_python_crash_exception(_frames(traceback))
        except Exception:
            pass
        self.previous_hook(kind, value, traceback)

    def close(self):
        if self.handle:
            self.library.msime_python_crash_stop(self.handle)
            self.handle = None
        if sys.excepthook is self.hook:
            sys.excepthook = self.previous_hook


def start(component, preferences=None, options=None):
    """解析参数后在主线程调用一次；源码树缺少安装身份时安静停用。"""
    global _capture, _preferences, _options, _source_names
    if _capture is not None:
        return
    _preferences = str(preferences) if preferences is not None else ""
    _options = str(options) if options is not None else ""
    try:
        from msime_voice_paths import CRASH_LIBRARY, EDITION
        if re.fullmatch(r"[a-z][a-z0-9]*", EDITION):
            _source_names = _source_names | {"msime-linux-%s-setup" % EDITION}
        root = Path(__file__).resolve().parent
        library = ctypes.CDLL(str(root / CRASH_LIBRARY))
        library.msime_python_crash_start.argtypes = [ctypes.c_char_p] * 3
        library.msime_python_crash_start.restype = ctypes.c_void_p
        library.msime_python_crash_stop.argtypes = [ctypes.c_void_p]
        library.msime_python_crash_stop.restype = None
        library.msime_python_crash_exception.argtypes = [ctypes.c_char_p]
        library.msime_python_crash_exception.restype = None
        handle = library.msime_python_crash_start(component.encode("ascii"),
                                                 os.fsencode(_preferences), os.fsencode(_options))
        if handle:
            _capture = _Capture(library, handle)
            atexit.register(_capture.close)
    except (ImportError, AttributeError, OSError, ValueError):
        pass


def worker_environment():
    """HTTP 子进程继承父进程实际偏好选择，不从请求内容或私有服务配置猜测同意。"""
    environment = dict(os.environ)
    environment["MSIME_CRASH_PARENT_PREFERENCES"] = _preferences
    environment["MSIME_CRASH_PARENT_OPTIONS"] = _options
    return environment


def start_worker(component):
    start(component, preferences=os.environ.get("MSIME_CRASH_PARENT_PREFERENCES") or None,
          options=os.environ.get("MSIME_CRASH_PARENT_OPTIONS") or None)
