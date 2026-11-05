"""Shared Unix provider startup ownership and stale socket recovery, plus the Volcano Ark model rules both providers apply."""
import errno
import fcntl
import os
import signal
import socket
import socketserver
import stat
import urllib.parse


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


def volcengine_successor_model(successors, endpoint, model):
    """火山方舟预设下的已下线模型对应的接替模型，没有就返回 None。

    与 client-core 的 `volcengine_successor_model` 同一条规则：`successors` 是导出到 msime_voice_provider_catalog 的 VOLCENGINE_SUCCESSOR_MODELS；只在接口地址留空或主机属于 volces.com 时换，指向别家网关的地址那边可能真有同名模型，不动。偏好读出时已经换了名，私有凭据文件里绑的还是旧名字，两边照同一张表换才对得上，请求也发接替模型。
    """
    endpoint = endpoint.strip()
    if endpoint:
        try:
            host = urllib.parse.urlsplit(endpoint).hostname
        except ValueError:
            return None
        if not host or (host != "volces.com" and not host.endswith(".volces.com")):
            return None
    return successors.get(model.strip())


def disables_ark_seed_thinking(provider, endpoint, model):
    """火山方舟上的豆包 Seed 默认开着深度思考，联想、释义和润色这类短请求要显式发 thinking.type = disabled。

    方舟深度思考文档（https://docs.volcengine.com/docs/82379/1449737 ，2026-11-05 查）列出的 doubao-seed-* 全部支持 disabled，同平台的别的模型不一定（glm-5-3-flash-260828 只支持 enabled），所以接口地址属于 volces.com（地址缺失时看 volcengine 标签）且模型属于这个系列才算。与 client-providers 的 `disables_ark_seed_thinking` 一致。
    """
    if not isinstance(endpoint, str) or not isinstance(model, str):
        return False
    if "://" in endpoint:
        try:
            host = urllib.parse.urlsplit(endpoint.strip()).hostname
        except ValueError:
            return False
        ark = bool(host) and (host == "volces.com" or host.endswith(".volces.com"))
    else:
        ark = provider == "volcengine"
    return ark and model.strip().lower().startswith("doubao-seed-")
