#!/usr/bin/env python3
"""IBus 宿主的使用上报：会话在启动时开始（文件 I/O，不联网），投递在注册 component 之后的后台线程里进行，端点挂起也不耽误注册和主循环；崩溃守护重启（--recovered）同样开始新会话，上一会话留下崩溃记录时补报 crash 和 session_crash；宿主收到崩溃信号时只把崩溃记录写到磁盘。

跑的是真的 msime-linux-ibus、真的 platforms/common/Telemetry.cpp 和它包装的 Host API。ibus-daemon 由一个普通 dbus-daemon 加 ibus-registration-bus 桩代替，桩只应答 RegisterComponent 并打出注册时刻和宿主的总线名。端点挂起期间，桩的 probe 模式向宿主的 IBusFactory 请求一个不存在的引擎：这个错误只能由宿主主循环回出来，所以投递一旦回到主线程同步执行，探测就会超时。投递经 HTTPS_PROXY（Host API 的 HTTP 客户端自己认）指到本地一个只 accept、从不回应的 TCP 监听，所以请求永远到不了 api.msime.app，而且对宿主来说就是一个挂起的端点。事件先排进 $XDG_STATE_HOME/msime/telemetry.json，内容从那里核对。

用法：ibus_startup_telemetry.py HOST REGISTRATION_BUS EXPECTED_VERSION
"""
import json
import os
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import threading
import time
from pathlib import Path

SKIP = 77


class HangingEndpoint:
    """Accepts every connection, records when and what the client sent first, and never answers."""

    def __init__(self):
        self.server = socket.socket()
        self.server.bind(("127.0.0.1", 0))
        self.server.listen()
        self.port = self.server.getsockname()[1]
        self.connections = []
        self.held = []
        threading.Thread(target=self.serve, daemon=True).start()

    def serve(self):
        while True:
            try:
                connection, _ = self.server.accept()
            except OSError:
                return
            accepted = time.monotonic()
            connection.settimeout(2)
            try:
                first = connection.recv(4096).decode(errors="replace")
            except OSError:
                first = ""
            self.held.append(connection)
            self.connections.append((accepted, first))

    def close(self):
        self.server.close()
        for connection in self.held:
            connection.close()


def wait_for(predicate, message, timeout=10.0):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if predicate():
            return
        time.sleep(0.02)
    raise AssertionError(message)


def stop(process):
    if process and process.poll() is None:
        process.send_signal(signal.SIGTERM)
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()


def run_host(host, registration_bus, scratch, recovered):
    """Start the host against a fresh bus and endpoint; returns (registration time, main loop probe, endpoint, host process, cleanup)."""
    bus_socket = scratch / "bus"
    # 自带一份最小的会话总线配置，与 GTestDBus 的做法相同：`--session` 读的是本机的 session.conf，
    # 没有 /etc 的环境（Nix 构建沙箱）里它不存在，总线起不来。stderr 不再丢掉，起不来时原因能看到。
    config = scratch / "bus.conf"
    config.write_text(
        "<busconfig><type>session</type>"
        f"<listen>unix:path={bus_socket}</listen>"
        '<policy context="default"><allow send_destination="*" eavesdrop="true"/>'
        '<allow eavesdrop="true"/><allow own="*"/></policy></busconfig>'
    )
    daemon = subprocess.Popen(
        ["dbus-daemon", f"--config-file={config}", "--nofork", "--print-address=1"],
        stdout=subprocess.PIPE, text=True,
    )
    processes = [daemon]
    endpoint = HangingEndpoint()

    def cleanup():
        for process in reversed(processes):
            stop(process)
        endpoint.close()

    try:
        address = daemon.stdout.readline().strip()
        assert address, "dbus-daemon printed no address"
        fake = subprocess.Popen([registration_bus, address], stdout=subprocess.PIPE, text=True)
        processes.append(fake)
        assert fake.stdout.readline().strip() == "ready", "registration bus did not start"
        registrations = []
        threading.Thread(
            target=lambda: [registrations.append((int(line.split()[1]) / 1e6, line.split()[2])) for line in fake.stdout if line.startswith("registered ")],
            daemon=True,
        ).start()

        options = scratch / "options/runtime-options.json"
        options.parent.mkdir()
        options.write_text('{"preferences":{}}')
        proxy = f"http://127.0.0.1:{endpoint.port}"
        environment = {
            "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
            "HOME": str(scratch / "home"),
            "XDG_CONFIG_HOME": str(scratch / "config"),
            "XDG_STATE_HOME": str(scratch / "state"),
            "XDG_RUNTIME_DIR": str(scratch),
            "IBUS_ADDRESS": address,
            "DBUS_SESSION_BUS_ADDRESS": address,
            # The Host API's HTTP client reads these itself, so the request goes to the hanging endpoint instead of api.msime.app.
            "HTTPS_PROXY": proxy, "https_proxy": proxy, "ALL_PROXY": proxy, "all_proxy": proxy,
        }
        arguments = [host] + (["--recovered"] if recovered else []) + [str(options)]
        launched = time.monotonic()
        log = scratch / "host.log"
        with log.open("w") as output:
            process = subprocess.Popen(arguments, env=environment, stdout=output, stderr=subprocess.STDOUT)
        processes.append(process)
        # Registration must not wait on the endpoint; well under the client's timeouts leaves room for a slow machine.
        wait_for(lambda: registrations or process.poll() is not None, "host never registered its component", timeout=10)
        assert registrations, ("host exited before registering", process.returncode, log.read_text())
        registered, sender = registrations[0]
        assert registered - launched < 2.5, ("registration waited", registered - launched)

        def probe():
            """Ask the host's factory for an unknown engine; returns the reply line from the stub."""
            return subprocess.run([registration_bus, address, "probe", sender], capture_output=True, text=True, timeout=10, check=True).stdout.strip()

        return registered, probe, endpoint, process, cleanup
    except BaseException:
        cleanup()
        raise


STATE = "state/msime"


def events(scratch):
    path = scratch / STATE / "telemetry.json"
    return json.loads(path.read_text()) if path.exists() else []


def kinds(scratch):
    return [event["kind"] for event in events(scratch)]


def crash_records(scratch):
    directory = scratch / STATE / "telemetry-crashes"
    return sorted(directory.glob("*.crash")) if directory.exists() else []


def main():
    if len(sys.argv) != 4:
        sys.exit(__doc__)
    host, registration_bus, version = sys.argv[1:]
    if not shutil.which("dbus-daemon"):
        print("dbus-daemon is not installed; skipping", file=sys.stderr)
        return SKIP

    # 正常启动：会话开始时排进当天的 active；注册先完成，然后后台线程才连端点，端点一直不回应，宿主照常活着。
    with tempfile.TemporaryDirectory() as name:
        scratch = Path(name)
        registered, probe, endpoint, process, cleanup = run_host(host, registration_bus, scratch, recovered=False)
        try:
            wait_for(lambda: endpoint.connections, "queued events were never sent")
            accepted, request = endpoint.connections[0]
            assert request.startswith("CONNECT api.msime.app:443 "), request
            assert accepted >= registered, ("telemetry ran before registration", accepted, registered)
            # While the request hangs, the main loop that serves ibus-daemon's CreateEngine and focus calls must still answer. A send moved back onto the main thread after registration passes every other check here: registration still comes first, the event is still queued and the process is still alive, blocked inside the request.
            reply = probe().split()
            assert reply[:1] == ["reply"] and reply[2:] == ["org.freedesktop.DBus.Error.Failed"], ("main loop did not answer within 1 s while the endpoint hung", reply)
            [event] = events(scratch)
            assert {key: event[key] for key in ("kind", "platform", "version")} == {
                "kind": "active", "platform": "linux", "version": version}, event
            install_id = event["install_id"]
            assert 16 <= len(install_id) <= 64, install_id
            assert event["id"].startswith(f"active-{install_id}-"), event
            assert set(event) == {"id", "kind", "platform", "version", "install_id"}, event
            assert (scratch / STATE / "telemetry-session.json").exists()
            # The request is still hanging and the host keeps running beside it.
            time.sleep(1)
            assert process.poll() is None, ("host exited while the endpoint hung", process.returncode)
            assert len(endpoint.connections) == 1, endpoint.connections
        finally:
            cleanup()

    # 崩溃守护重启（--recovered）：上一会话留下了崩溃记录，所以补报 session_crash 和 crash，崩溃记录里的目录只剩文件名；同样排进 active。
    with tempfile.TemporaryDirectory() as name:
        scratch = Path(name)
        state = scratch / STATE
        (state / "telemetry-crashes").mkdir(parents=True)
        previous = "6f1c2a7e-0d35-4c0b-9a4e-2f8d1b3c4e5f"
        (state / "telemetry-session.json").write_text(json.dumps(
            {"id": previous, "platform": "linux", "version": "0.0.1", "started_at_unix_ms": 1}))
        (state / "telemetry-crashes" / f"{previous}.crash").write_text(
            "SIGSEGV: segmentation fault\n/home/someone/.local/lib/libmsime_host_api.so(+0x1f) [0x7f00]\n")
        _, _, endpoint, process, cleanup = run_host(host, registration_bus, scratch, recovered=True)
        try:
            wait_for(lambda: endpoint.connections, "queued events were never sent")
            assert process.poll() is None, ("recovered host exited", process.returncode)
            queued = events(scratch)
            assert [event["kind"] for event in queued] == ["session_crash", "crash", "active"], queued
            session_crash, crash, _ = queued
            assert session_crash["id"] == f"session-{previous}" and session_crash["version"] == "0.0.1", session_crash
            assert crash["message"] == "SIGSEGV: segmentation fault", crash
            assert crash["stack"] == "libmsime_host_api.so(+0x1f) [0x7f00]", crash
            assert "someone" not in json.dumps(queued)
            assert not crash_records(scratch)
        finally:
            cleanup()

    # 宿主收到崩溃信号：处理函数只把这次会话的崩溃记录写到磁盘（第一行是信号摘要），然后照默认动作退出，不联网。
    with tempfile.TemporaryDirectory() as name:
        scratch = Path(name)
        _, _, endpoint, process, cleanup = run_host(host, registration_bus, scratch, recovered=False)
        try:
            wait_for(lambda: endpoint.connections, "queued events were never sent")
            connections = len(endpoint.connections)
            process.send_signal(signal.SIGSEGV)
            process.wait(timeout=10)
            assert process.returncode == -signal.SIGSEGV, process.returncode
            [record] = crash_records(scratch)
            assert record.read_text().startswith("SIGSEGV: segmentation fault\n"), record.read_text()
            assert len(endpoint.connections) == connections, "the crash path used the network"
        finally:
            cleanup()
    return 0


if __name__ == "__main__":
    sys.exit(main())
