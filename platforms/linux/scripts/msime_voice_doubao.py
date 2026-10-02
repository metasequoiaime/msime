"""Doubao streaming transport, based on MSIME-Windows 7fa6fb1a7862c5ca1541b9cb839d9bea3a06e2c6.

No request, audio, transcript, credential, or remote error body is logged.
"""
import gzip
import json
import logging
import queue
import socket
import struct
import threading
import time
import uuid
import zlib


CHUNK_BYTES = 6400  # 200ms of 16kHz signed 16-bit mono PCM.
MAX_RESPONSE = 1024 * 1024


def normalize_doubao_auth_mode(mode):
    """Doubao's console generation: "legacy" (App ID plus Access Token) only when named, otherwise "api_key"."""
    normalized = mode.lower() if isinstance(mode, str) and mode.isascii() else ""
    return "legacy" if normalized == "legacy" else "api_key"


def doubao_headers(config, request_id):
    app_key = config.get("app_key", "")
    token = config.get("token", "")
    resource_id = config.get("resource_id", "")
    if not all(isinstance(value, str) for value in (app_key, token, resource_id)):
        raise ValueError("invalid Doubao authentication configuration")
    app_key = app_key.strip(" \t\r\n")
    token = token.strip(" \t\r\n")
    resource_id = resource_id.strip(" \t\r\n")
    mode = normalize_doubao_auth_mode(config.get("doubao_auth_mode"))
    headers = {"X-Api-Resource-Id": resource_id,
               "X-Api-Request-Id": request_id}
    if mode == "legacy":
        if not app_key:
            raise ValueError("legacy Doubao authentication requires an App ID")
        headers.update({"X-Api-App-Key": app_key,
                        "X-Api-Access-Key": token})
    else:
        headers["X-Api-Key"] = token
    return headers


# websockets 15.0 is the first release whose synchronous client takes ping_interval and ping_timeout; 13.x and 14.x forward unknown keywords to socket.create_connection, so an older package would only fail once a recording is already connecting.
WEBSOCKETS_MIN_MAJOR = 15
WEBSOCKETS_REQUIRED = "websockets>=%d with sync client required" % WEBSOCKETS_MIN_MAJOR
# Every keyword DoubaoStream.run passes to connect().
CONNECT_ARGUMENTS = frozenset((
    "additional_headers", "user_agent_header", "open_timeout", "close_timeout", "ping_interval",
    "ping_timeout", "max_size", "max_queue", "compression", "logger", "create_connection"))


def websocket_dependency():
    """Return the synchronous client, or raise RuntimeError when the installed websockets cannot run the Doubao transport."""
    import inspect
    from importlib.metadata import version
    try:
        major = int(version("websockets").split(".")[0])
        from websockets.sync.client import ClientConnection, connect
        accepted = inspect.signature(connect).parameters
        receive = inspect.signature(ClientConnection.recv).parameters
    except Exception as error:
        raise RuntimeError(WEBSOCKETS_REQUIRED) from error
    if major < WEBSOCKETS_MIN_MAJOR or not CONNECT_ARGUMENTS <= accepted.keys() or "timeout" not in receive:
        raise RuntimeError(WEBSOCKETS_REQUIRED)
    return ClientConnection, connect


def packet(kind, flags, sequence, payload):
    compressed = gzip.compress(payload, mtime=0)
    return bytes((0x11, kind << 4 | flags, 0x11, 0)) + struct.pack(
        ">iI", sequence, len(compressed)) + compressed


def parse_response(message):
    if not isinstance(message, bytes) or not 4 <= len(message) <= MAX_RESPONSE:
        raise ValueError("invalid Doubao message")
    header = (message[0] & 15) * 4
    if message[0] >> 4 != 1 or header < 4 or header > len(message):
        raise ValueError("invalid Doubao header")
    kind, flags = message[1] >> 4, message[1] & 15
    serialization, compression = message[2] >> 4, message[2] & 15
    offset = header + (4 if flags & 1 else 0) + (4 if flags & 4 else 0)
    if kind == 15:
        # Do not expose the server's diagnostic body to logs or the host.
        raise ValueError("Doubao rejected request")
    if kind != 9 or offset + 4 > len(message) or serialization != 1:
        raise ValueError("invalid Doubao response")
    length = struct.unpack_from(">I", message, offset)[0]
    offset += 4
    if length != len(message) - offset:
        raise ValueError("truncated Doubao payload")
    payload = message[offset:]
    if compression == 1:
        decoder = zlib.decompressobj(16 + zlib.MAX_WBITS)
        payload = decoder.decompress(payload, MAX_RESPONSE + 1)
        if (len(payload) > MAX_RESPONSE or not decoder.eof or
            decoder.unconsumed_tail or decoder.unused_data):
            raise ValueError("invalid or oversized Doubao gzip")
    elif compression != 0:
        raise ValueError("unsupported Doubao compression")
    value = json.loads(payload)
    body = value.get("payload_msg", value)
    result = body.get("result", {})
    text = result.get("text", "")
    if not isinstance(text, str):
        raise ValueError("invalid Doubao transcript")
    return text, bool(flags & 2)


def request_body(options):
    request = {"model_name": "bigmodel", "result_type": "full", "show_utterances": False}
    for key, default in (("enable_itn", True), ("enable_punc", True), ("enable_ddc", False)):
        value = options.get("doubao_" + key, default)
        if type(value) is not bool:
            raise ValueError("invalid Doubao option")
        request[key] = value
    boosting = options.get("doubao_boosting_table_id", "")
    if not isinstance(boosting, str) or len(boosting.encode()) > 512:
        raise ValueError("invalid Doubao boosting table")
    if boosting:
        request["corpus"] = {"boosting_table_id": boosting}
    return json.dumps({"user": {"uid": "metasequoia-ime"},
                       "audio": {"format": "pcm", "codec": "raw", "rate": 16000,
                                 "bits": 16, "channel": 1},
                       "request": request}).encode()


def _placeholder(value):
    return (not value or
            (len(value) >= 2 and value.startswith("<") and value.endswith(">")) or
            value.startswith("FAKESECRET_"))


def doubao_auth_headers(config, options):
    """Choose Doubao headers without logging or exposing credential values."""
    app_key = config.get("app_key", "")
    token = config.get("token", "")
    mode = options.get("doubao_auth_mode", "")
    if not isinstance(mode, str) or not isinstance(app_key, str) or not isinstance(token, str):
        raise ValueError("invalid Doubao authentication configuration")
    if normalize_doubao_auth_mode(mode) == "legacy":
        if _placeholder(app_key):
            raise ValueError("Doubao legacy authentication requires an App ID")
        return {"X-Api-App-Key": app_key, "X-Api-Access-Key": token}
    # New-console API Key authentication deliberately ignores a stale App ID.
    return {"X-Api-Key": token}


class DoubaoStream:
    def __init__(self, config, options, cancelled):
        self.config = config
        self.auth_headers = doubao_auth_headers(config, options)
        self.initial = request_body(options)
        self.cancelled = cancelled
        self.closed = threading.Event()
        self.done = threading.Event()
        self.audio = queue.Queue(maxsize=50)  # At most ten seconds awaiting upload.
        self.pending = bytearray()
        self.lock = threading.Lock()
        self.transport = None
        self.text = ""
        self.failed = False
        self.worker = threading.Thread(target=self.run, daemon=True)
        self.worker.start()

    def feed(self, chunk):
        if self.cancelled.is_set() or self.closed.is_set() or self.done.is_set():
            return
        self.pending.extend(chunk)
        while len(self.pending) >= CHUNK_BYTES:
            self.audio.put_nowait((bytes(self.pending[:CHUNK_BYTES]), False))
            del self.pending[:CHUNK_BYTES]

    def latest(self):
        with self.lock:
            if self.failed:
                raise ValueError("Doubao recognition failed")
            return self.text

    def finish(self, tick):
        if not self.done.is_set() and not self.cancelled.is_set():
            final = bytes(self.pending[:len(self.pending) // 2 * 2])
            self.pending.clear()
            self.audio.put_nowait((final, True))
        deadline = time.monotonic() + 30
        while not self.done.wait(0.1):
            if self.cancelled.is_set() or time.monotonic() >= deadline:
                self.close()
                if self.cancelled.is_set():
                    return ""
                raise TimeoutError("Doubao final response timeout")
            tick()
        return "" if self.cancelled.is_set() else self.latest()

    def close(self):
        self.closed.set()
        with self.lock:
            transport = self.transport
        if transport:
            # Interrupt a blocked send without waiting for the send lock or
            # a WebSocket close handshake. TLS still owns certificate checks.
            try:
                transport.shutdown(socket.SHUT_RDWR)
            except OSError:
                pass
        self.worker.join(timeout=12)

    def run(self):
        received_final = threading.Event()
        try:
            connection_type, connect = websocket_dependency()
            headers = {"X-Api-Resource-Id": self.config["resource_id"],
                       "X-Api-Request-Id": str(uuid.uuid4())}
            headers.update(self.auth_headers)
            logger = logging.Logger("msime.voice.doubao")
            logger.disabled = True
            logger.propagate = False

            def connection_factory(sock, protocol, **kwargs):
                with self.lock:
                    self.transport = sock
                if self.closed.is_set() or self.cancelled.is_set():
                    sock.close()
                    raise ValueError("cancelled")
                return connection_type(sock, protocol, **kwargs)

            # The synchronous client rejects unsuccessful upgrades; it doesn't
            # follow redirects with these endpoint-bound authentication headers.
            with connect(self.config["endpoint"], additional_headers=headers,
                         user_agent_header="MSIME-Client", open_timeout=10,
                         close_timeout=1, ping_interval=10, ping_timeout=10,
                         max_size=MAX_RESPONSE, max_queue=4, compression=None,
                         logger=logger, create_connection=connection_factory) as websocket:
                if self.cancelled.is_set() or self.closed.is_set():
                    return
                websocket.send(packet(1, 1, 1, self.initial))
                receiver_stopped = threading.Event()

                def receive():
                    last_response = time.monotonic()
                    try:
                        while not receiver_stopped.is_set() and not self.closed.is_set() and not self.cancelled.is_set():
                            try:
                                message = websocket.recv(timeout=0.1)
                            except TimeoutError:
                                if time.monotonic() - last_response >= 30:
                                    raise TimeoutError("Doubao response timeout")
                                continue
                            text, final = parse_response(message)
                            last_response = time.monotonic()
                            if text:
                                with self.lock:
                                    self.text = text
                            if final:
                                received_final.set()
                                return
                    except Exception:
                        if not receiver_stopped.is_set() and not self.closed.is_set() and not self.cancelled.is_set():
                            with self.lock:
                                self.failed = True
                    finally:
                        receiver_stopped.set()
                        # A final reply or failed receive must also release a
                        # sender blocked in upload backpressure.
                        try:
                            self.transport.shutdown(socket.SHUT_RDWR)
                        except OSError:
                            pass

                receiver = threading.Thread(target=receive, daemon=True)
                receiver.start()
                sequence = 2
                finish_deadline = None
                try:
                    # Like the Windows duplex transport, one receiver remains
                    # active independently of audio sends and their backpressure.
                    while (not receiver_stopped.is_set() and not self.closed.is_set()
                           and not self.cancelled.is_set()):
                        if finish_deadline is not None:
                            if time.monotonic() >= finish_deadline:
                                raise TimeoutError("Doubao final response timeout")
                            receiver_stopped.wait(0.05)
                            continue
                        try:
                            chunk, final = self.audio.get(timeout=0.05)
                        except queue.Empty:
                            continue
                        websocket.send(packet(2, 3 if final else 1, -sequence if final else sequence, chunk))
                        sequence += 1
                        if final:
                            finish_deadline = time.monotonic() + 30
                finally:
                    receiver_stopped.set()
                    try:
                        self.transport.shutdown(socket.SHUT_RDWR)
                    except OSError:
                        pass
                    receiver.join()
        except Exception:
            if not received_final.is_set() and not self.cancelled.is_set() and not self.closed.is_set():
                with self.lock:
                    self.failed = True
        finally:
            with self.lock:
                self.transport = None
            self.done.set()
