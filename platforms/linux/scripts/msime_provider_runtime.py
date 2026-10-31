"""Shared Unix provider startup ownership and stale socket recovery."""
import errno
import fcntl
import os
import signal
import socket
import socketserver
import stat


class _StopServing(Exception):
    """仅在服务循环的安全点退出，不从信号处理器抛出。"""


class StoppableUnixStreamServer(socketserver.ThreadingMixIn, socketserver.UnixStreamServer):
    stop_requested = False

    def service_actions(self):
        super().service_actions()
        if self.stop_requested:
            raise _StopServing()

    def serve_until_stopped(self):
        """供独立进程主入口调用；停止处理器持续保护随后执行的资源清理。"""
        self.stop_requested = False

        def stop(signum, frame):
            # 主线程可能正持有 Python 的线程注册锁；信号中不能再建线程或获取锁。
            self.stop_requested = True

        signal.signal(signal.SIGTERM, stop)
        try:
            self.serve_forever()
        except (_StopServing, KeyboardInterrupt):
            pass


def claim_socket(path):
    """Serialize startup and recover only an owned, unresponsive socket."""
    lock = os.open(str(path) + ".lock", os.O_CREAT | os.O_RDWR | os.O_NOFOLLOW, 0o600)
    try:
        info = os.fstat(lock)
        if info.st_uid != os.getuid() or not stat.S_ISREG(info.st_mode) or info.st_mode & 0o077:
            raise ValueError("invalid service lock")
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        try:
            before = path.lstat()
        except FileNotFoundError:
            return lock
        if before.st_uid != os.getuid() or not stat.S_ISSOCK(before.st_mode):
            raise ValueError("socket path is occupied")
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as probe:
            probe.settimeout(0.5)
            try:
                probe.connect(str(path))
            except OSError as error:
                if error.errno != errno.ECONNREFUSED:
                    raise ValueError("socket availability is unknown") from None
            else:
                raise ValueError("provider service is already running")
        after = path.lstat()
        if (before.st_dev, before.st_ino) != (after.st_dev, after.st_ino):
            raise ValueError("socket changed during startup")
        path.unlink()
        return lock
    except Exception:
        os.close(lock)
        raise



def inherited_socket(path):
    """Return the listening socket systemd passed for path, or None when the provider was started directly."""
    if os.environ.get("LISTEN_PID") != str(os.getpid()):
        return None
    count = os.environ.get("LISTEN_FDS")
    # The variables name this process only; the HTTP workers the providers spawn must not believe fd 3 is theirs.
    for name in ("LISTEN_PID", "LISTEN_FDS", "LISTEN_FDNAMES"):
        os.environ.pop(name, None)
    if count != "1":
        raise ValueError("expected exactly one inherited socket")
    listener = socket.socket(fileno=3)
    try:
        if (listener.family != socket.AF_UNIX or listener.type != socket.SOCK_STREAM or
                listener.getsockname() != str(path)):
            raise ValueError("inherited socket does not match the provider path")
        listener.set_inheritable(False)
    except Exception:
        listener.close()
        raise
    return listener


def open_server(server_class, path, handler):
    """Bind the provider server, returning it with the startup lock.

    Under systemd socket activation the lock is None: systemd owns the socket path, keeps it across provider restarts and removes it itself, so the provider must neither claim nor unlink it.
    """
    listener = inherited_socket(path)
    if listener is not None:
        server = server_class(str(path), handler, bind_and_activate=False)
        server.socket.close()
        server.socket = listener
        return server, None
    lock = claim_socket(path)
    try:
        return server_class(str(path), handler), lock
    except Exception:
        os.close(lock)
        raise
