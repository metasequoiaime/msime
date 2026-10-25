"""跨平台串行执行仓库契约检查中的 Cargo 导出器。"""

from contextlib import contextmanager
import errno
import os
from pathlib import Path
import tempfile
import time


@contextmanager
def contract_cargo_lock(path=None):
    lock_path = Path(path) if path is not None else Path(tempfile.gettempdir()) / "msime-contract-cargo.lock"
    with lock_path.open("a+b") as lock:
        if os.name == "nt":
            import msvcrt

            # Windows 可锁定文件末尾以外的区域，空锁文件无需并发写入初始化字节。
            lock.seek(0)
            while True:
                try:
                    msvcrt.locking(lock.fileno(), msvcrt.LK_NBLCK, 1)
                    break
                except OSError as error:
                    if error.errno != errno.EACCES:
                        raise
                    time.sleep(0.05)
        else:
            import fcntl

            fcntl.flock(lock.fileno(), fcntl.LOCK_EX)
        try:
            yield
        finally:
            if os.name == "nt":
                lock.seek(0)
                msvcrt.locking(lock.fileno(), msvcrt.LK_UNLCK, 1)
            else:
                fcntl.flock(lock.fileno(), fcntl.LOCK_UN)
