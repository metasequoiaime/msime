"""On-device recognition for msime-linux-voice-provider through the msime-voice-local helper.

The helper (shared/voice/LocalAsrHelper.cpp) loads the sherpa-onnx runtime and the model in its own process and speaks one JSON object per line on stdin and stdout. The service keeps one idle helper warm between recordings, so the model the helper caches stays loaded for the next dictation; a helper that did not end its session cleanly is killed instead of reused.
"""

import base64
import ctypes
import json
import os
import queue
import stat
import subprocess
import sys
import threading
import time
from pathlib import Path


MANIFEST = "msime-model.json"
MAX_MANIFEST = 1024 * 1024
MAX_MODEL_PATH = 4096
MAX_LINE = 1024 * 1024
# 100 ms of 16 kHz mono 16-bit audio per audio message.
CHUNK_BYTES = 3200
HELLO_TIMEOUT = 10
# Offline models decode the whole recording after it ends, and a first dictation also loads the model, so the final answer gets far longer than a cloud stream's.
FINAL_TIMEOUT = 120
CANCEL_TIMEOUT = 3
# The helper exits on its own after this many idle seconds; the pool retires a warm helper earlier so a checkout never races that exit.
HELPER_IDLE_EXIT = 600
POOL_IDLE_LIMIT = 540
MAX_HOTWORDS = 1000


class LocalUnavailable(RuntimeError):
    """The helper or the sherpa-onnx runtime it loads is missing: an installation problem, reported as voice_dependency_missing."""


def model_manifest(path):
    """Validate `asr_model_path` as an installed model directory and return its msime-model.json. Raise ValueError for anything else, including a Whisper model file, which the Linux service does not run."""
    if (not isinstance(path, str) or not path or len(path.encode("utf-8", "surrogatepass")) > MAX_MODEL_PATH
            or any(ord(c) < 32 or 127 <= ord(c) <= 159 for c in path) or not os.path.isabs(path)):
        raise ValueError("invalid local model path")
    if not stat.S_ISDIR(os.stat(path, follow_symlinks=False).st_mode):
        raise ValueError("local model path is not a model directory")
    descriptor = os.open(os.path.join(path, MANIFEST), os.O_RDONLY | os.O_NONBLOCK | os.O_CLOEXEC | os.O_NOFOLLOW)
    with os.fdopen(descriptor, "rb") as source:
        if not stat.S_ISREG(os.fstat(source.fileno()).st_mode):
            raise ValueError("invalid local model manifest")
        raw = source.read(MAX_MANIFEST + 1)
    if len(raw) > MAX_MANIFEST:
        raise ValueError("invalid local model manifest")
    manifest = json.loads(raw.decode("utf-8"))
    if not isinstance(manifest, dict):
        raise ValueError("invalid local model manifest")
    return manifest


def parse_hotwords(value):
    """Decode the `voice_hotwords` option the native hosts send: one `text<TAB>pinyin` line per user dictionary word, heaviest first."""
    if not isinstance(value, str):
        return []
    hotwords = []
    seen = set()
    for line in value.split("\n"):
        text, separator, pinyin = line.partition("\t")
        text = text.strip()
        pinyin = pinyin.strip()
        if not text or not separator or text in seen:
            continue
        seen.add(text)
        hotwords.append({"text": text, "pinyin": pinyin})
        if len(hotwords) >= MAX_HOTWORDS:
            break
    return hotwords


class HotwordCorrector:
    """msime_client_voice_hotword_correct from the host library, loaded on first use. Models whose manifest says "hotwords": "pinyin" cannot bias decoding, so their transcript is corrected against the hotwords afterwards; without the library the transcript is kept as recognised."""

    def __init__(self, library):
        self.library = library
        self.lock = threading.Lock()
        self.handle = None
        self.failed = False

    def load(self):
        with self.lock:
            if self.handle is None and not self.failed:
                try:
                    handle = ctypes.CDLL(str(self.library))
                    handle.msime_client_voice_hotword_correct.argtypes = (ctypes.c_char_p, ctypes.c_size_t)
                    handle.msime_client_voice_hotword_correct.restype = ctypes.c_void_p
                    handle.msime_client_string_free.argtypes = (ctypes.c_void_p,)
                    handle.msime_client_string_free.restype = None
                    self.handle = handle
                except (OSError, AttributeError, TypeError):
                    self.failed = True
            return self.handle

    def correct(self, text, hotwords):
        if not text or not hotwords or self.library is None:
            return text
        handle = self.load()
        if handle is None:
            return text
        request = json.dumps({"text": text, "hotwords": hotwords}, ensure_ascii=False).encode()
        if len(request) > 1024 * 1024:
            return text
        result = handle.msime_client_voice_hotword_correct(request, len(request))
        if not result:
            return text
        try:
            document = json.loads(ctypes.string_at(result).decode("utf-8"))
        finally:
            handle.msime_client_string_free(result)
        value = document.get("value") if isinstance(document, dict) and document.get("ok") else None
        corrected = value.get("text") if isinstance(value, dict) else None
        return corrected if isinstance(corrected, str) and corrected else text


class Helper:
    """One running msime-voice-local process and the thread reading its answers."""

    def __init__(self, command):
        self.process = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                        stderr=None, close_fds=True)
        self.events = queue.Queue()
        self.write_lock = threading.Lock()
        self.sessions = 0
        self.idle_since = time.monotonic()
        self.reader = threading.Thread(target=self.read, daemon=True)
        self.reader.start()

    def read(self):
        try:
            while True:
                line = self.process.stdout.readline(MAX_LINE + 1)
                if not line:
                    break
                if len(line) > MAX_LINE:
                    continue
                try:
                    event = json.loads(line)
                except ValueError:
                    continue
                if isinstance(event, dict):
                    self.events.put(event)
        except (OSError, ValueError):
            pass
        self.events.put({"type": "exit"})

    def send(self, message):
        data = json.dumps(message, ensure_ascii=False).encode() + b"\n"
        with self.write_lock:
            self.process.stdin.write(data)
            self.process.stdin.flush()

    def alive(self):
        return self.process.poll() is None

    def hello(self):
        try:
            event = self.events.get(timeout=HELLO_TIMEOUT)
        except queue.Empty:
            raise LocalUnavailable("msime-voice-local did not start") from None
        if event.get("type") != "hello":
            raise LocalUnavailable("msime-voice-local exited before it started")
        if not event.get("available"):
            error = event.get("error")
            raise LocalUnavailable(error if isinstance(error, str) and error else "sherpa-onnx runtime unavailable")

    def kill(self):
        try:
            if self.process.poll() is None:
                self.process.kill()
            self.process.wait(timeout=5)
        except (OSError, subprocess.TimeoutExpired):
            pass
        for pipe in (self.process.stdin, self.process.stdout):
            try:
                pipe.close()
            except OSError:
                pass


class HelperPool:
    """Hands out helper processes and keeps at most one idle, warm one."""

    def __init__(self, helper, runtime=None):
        self.helper = Path(helper) if helper else None
        self.runtime = runtime
        self.lock = threading.Lock()
        self.idle = None

    def command(self):
        command = [str(self.helper), "--idle-exit", str(HELPER_IDLE_EXIT)]
        if self.runtime:
            command += ["--runtime", str(self.runtime)]
        return command

    def acquire(self):
        with self.lock:
            helper, self.idle = self.idle, None
        if helper is not None:
            if helper.alive() and time.monotonic() - helper.idle_since < POOL_IDLE_LIMIT:
                return helper
            helper.kill()
        if self.helper is None or not os.access(self.helper, os.X_OK):
            raise LocalUnavailable("msime-voice-local is not installed")
        try:
            helper = Helper(self.command())
        except OSError as error:
            raise LocalUnavailable("msime-voice-local cannot start: %s" % error) from None
        try:
            helper.hello()
        except BaseException:
            helper.kill()
            raise
        return helper

    def release(self, helper, reusable):
        if reusable and helper.alive():
            helper.idle_since = time.monotonic()
            with self.lock:
                if self.idle is None:
                    self.idle = helper
                    return
        helper.kill()

    def close(self):
        with self.lock:
            helper, self.idle = self.idle, None
        if helper is not None:
            helper.kill()


class LocalStream:
    """The recognizer interface the voice service drives (feed, latest, done, finish, close), over one helper session."""

    def __init__(self, pool, model, language, hotwords, cancelled):
        self.pool = pool
        self.cancelled = cancelled
        self.done = threading.Event()
        self.text = ""
        self.failed = None
        self.finished = False
        self.closed = False
        self.pending = bytearray()
        self.helper = pool.acquire()
        self.helper.sessions += 1
        self.id = self.helper.sessions
        # Stale answers of an earlier session never reach this one: a helper returns to the pool only after its session ended with a terminal event.
        try:
            self.helper.send({"op": "start", "id": self.id, "model": model, "language": language,
                              "hotwords": [word["text"] for word in hotwords], "threads": 0})
        except OSError:
            self.pool.release(self.helper, False)
            self.helper = None
            raise

    def drain(self, timeout=0):
        """Apply the helper's answers; return True once the session has ended."""
        deadline = time.monotonic() + timeout
        while True:
            remaining = deadline - time.monotonic()
            try:
                event = (self.helper.events.get(timeout=remaining) if remaining > 0
                         else self.helper.events.get_nowait())
            except queue.Empty:
                return self.done.is_set()
            kind = event.get("type")
            if kind == "exit":
                if self.failed is None:
                    print("msime-linux-voice-provider: msime-voice-local exited during a recording", file=sys.stderr, flush=True)
                self.failed = self.failed or "msime-voice-local exited"
                self.done.set()
                return True
            if event.get("id") != self.id:
                continue
            text = event.get("text")
            if kind == "partial" and isinstance(text, str):
                self.text = text
            elif kind == "final":
                self.text = text if isinstance(text, str) else ""
                self.finished = True
                self.done.set()
                return True
            elif kind in ("error", "cancelled"):
                message = event.get("message")
                self.failed = message if isinstance(message, str) and message else kind
                if kind == "error":
                    # The helper's diagnostic is English text about the runtime or the model; the host only shows a generic failure, so the journal is where it can be read.
                    print("msime-linux-voice-provider: local recognition failed: %s" % self.failed[:512],
                          file=sys.stderr, flush=True)
                self.done.set()
                return True

    def feed(self, chunk):
        if self.cancelled.is_set() or self.done.is_set() or self.closed:
            return
        self.pending.extend(chunk)
        while len(self.pending) >= CHUNK_BYTES and not self.done.is_set():
            self.send_audio(bytes(self.pending[:CHUNK_BYTES]))
            del self.pending[:CHUNK_BYTES]

    def send(self, message):
        """Write to the helper; a helper that died mid-recording fails the session like an exit event instead of raising from the capture loop."""
        try:
            self.helper.send(message)
        except OSError:
            if self.failed is None:
                print("msime-linux-voice-provider: msime-voice-local exited during a recording", file=sys.stderr, flush=True)
            self.failed = "msime-voice-local exited"
            self.done.set()

    def send_audio(self, data):
        # The helper reads stdin on its own thread into an unbounded queue, so this write does not wait for decoding.
        self.send({"op": "audio", "pcm16": base64.b64encode(data).decode("ascii")})

    def latest(self):
        self.drain()
        if self.failed is not None and not self.cancelled.is_set():
            raise ValueError("local recognition failed: %s" % self.failed)
        return self.text

    def finish(self, tick):
        if not self.done.is_set() and not self.cancelled.is_set():
            tail = bytes(self.pending[:len(self.pending) // 2 * 2])
            self.pending.clear()
            if tail:
                self.send_audio(tail)
            if not self.done.is_set():
                self.send({"op": "finish"})
        deadline = time.monotonic() + FINAL_TIMEOUT
        while not self.drain(0.1):
            if self.cancelled.is_set():
                return ""
            if time.monotonic() >= deadline:
                raise TimeoutError("local recognition final timeout")
            tick()
        if self.cancelled.is_set():
            return ""
        return self.latest()

    def close(self):
        if self.closed or self.helper is None:
            return
        self.closed = True
        reusable = self.done.is_set() and self.failed != "msime-voice-local exited"
        if not self.done.is_set():
            # Stop a decode still in progress and wait briefly for the helper to confirm, so it can stay warm for the next recording.
            try:
                self.helper.send({"op": "cancel"})
                reusable = self.drain(CANCEL_TIMEOUT) and self.failed != "msime-voice-local exited"
            except OSError:
                reusable = False
        self.pool.release(self.helper, reusable)
        self.helper = None
