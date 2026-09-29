#!/usr/bin/env python3
"""IBus 宿主的启动上报：注册 component 之后才在后台线程发出，端点挂起也不耽误注册和主循环；守护进程崩溃重启（--recovered）时不发；上报的版本是构建版本。

跑的是真的 msime-linux-ibus 和真的 platforms/common/Telemetry.cpp。ibus-daemon 由一个普通 dbus-daemon 加 ibus-registration-bus 桩代替，桩只应答 RegisterComponent 并打出注册时刻和宿主的总线名。端点挂起期间，桩的 probe 模式向宿主的 IBusFactory 请求一个不存在的引擎：这个错误只能由宿主主循环回出来，所以上报一旦回到主线程同步执行，探测就会超时。上报端点经 libcurl 自己认的 HTTPS_PROXY 指到本地一个只 accept、从不回应的 TCP 监听，所以请求永远到不了 api.msime.app，而且对宿主来说就是一个挂起的端点。事件在发送前先写进 $XDG_STATE_HOME/msime/telemetry.json，版本号从那里核对。

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
    daemon = subprocess.Popen(
        ["dbus-daemon", "--session", "--nofork", "--print-address=1", f"--address=unix:path={bus_socket}"],
        stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True,
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
        if not recovered:
            queue = scratch / "state/msime"
            queue.mkdir(parents=True)
            (queue / "telemetry.json").write_text(json.dumps([
                {"id": f"old-{index}", "kind": "download", "platform": "fixture", "version": "0"}
                for index in range(64)
            ] + [{"id": "oversized", "kind": "download", "platform": "fixture",
                  "version": "0", "message": "x" * (1024 * 1024)}]))
        proxy = f"http://127.0.0.1:{endpoint.port}"
        environment = {
            "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
            "HOME": str(scratch / "home"),
            "XDG_CONFIG_HOME": str(scratch / "config"),
            "XDG_STATE_HOME": str(scratch / "state"),
            "XDG_RUNTIME_DIR": str(scratch),
            "IBUS_ADDRESS": address,
            "DBUS_SESSION_BUS_ADDRESS": address,
            # libcurl reads these itself, so the request goes to the hanging endpoint instead of api.msime.app.
            "HTTPS_PROXY": proxy, "https_proxy": proxy, "ALL_PROXY": proxy, "all_proxy": proxy,
        }
        arguments = [host] + (["--recovered"] if recovered else []) + [str(options)]
        launched = time.monotonic()
        log = scratch / "host.log"
        with log.open("w") as output:
            process = subprocess.Popen(arguments, env=environment, stdout=output, stderr=subprocess.STDOUT)
        processes.append(process)
        # Registration must not wait on the endpoint. The old synchronous send held it for the 3 s connect timeout; well under that leaves room for a slow machine.
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


def events(scratch):
    path = scratch / "state/msime/telemetry.json"
    return json.loads(path.read_text()) if path.exists() else []


def main():
    if len(sys.argv) != 4:
        sys.exit(__doc__)
    host, registration_bus, version = sys.argv[1:]
    if not shutil.which("dbus-daemon"):
        print("dbus-daemon is not installed; skipping", file=sys.stderr)
        return SKIP

    # 正常启动：注册先完成，然后后台线程才连端点，端点一直不回应，宿主照常活着。
    with tempfile.TemporaryDirectory() as name:
        scratch = Path(name)
        registered, probe, endpoint, process, cleanup = run_host(host, registration_bus, scratch, recovered=False)
        try:
            wait_for(lambda: endpoint.connections, "startup event was never sent")
            accepted, request = endpoint.connections[0]
            assert request.startswith("CONNECT api.msime.app:443 "), request
            assert accepted >= registered, ("telemetry ran before registration", accepted, registered)
            # While the request hangs, the main loop that serves ibus-daemon's CreateEngine and focus calls must still answer. A send moved back onto the main thread after registration passes every other check here: registration still comes first, the event is still queued and the process is still alive, blocked inside curl.
            reply = probe().split()
            assert reply[:1] == ["reply"] and reply[2:] == ["org.freedesktop.DBus.Error.Failed"], ("main loop did not answer within 1 s while the endpoint hung", reply)
            wait_for(lambda: events(scratch), "startup event was not queued")
            [event] = events(scratch)
            assert {key: event[key] for key in ("kind", "platform", "version")} == {
                "kind": "download", "platform": "linux", "version": version}, event
            assert set(event) == {"id", "kind", "platform", "version"}, event
            # The request is still hanging and the host keeps running beside it.
            time.sleep(1)
            assert process.poll() is None, ("host exited while the endpoint hung", process.returncode)
            assert len(endpoint.connections) == 1, endpoint.connections
        finally:
            cleanup()

    # 守护进程重启（--recovered）：同样注册，但不发也不落盘任何启动事件。
    with tempfile.TemporaryDirectory() as name:
        scratch = Path(name)
        _, _, endpoint, process, cleanup = run_host(host, registration_bus, scratch, recovered=True)
        try:
            time.sleep(3)
            assert process.poll() is None, ("recovered host exited", process.returncode)
            assert not endpoint.connections, endpoint.connections
            assert not events(scratch), events(scratch)
        finally:
            cleanup()
    return 0


if __name__ == "__main__":
    sys.exit(main())
