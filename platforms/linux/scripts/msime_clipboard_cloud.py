"""云剪贴板的单进程同步状态；不保存正文队列。"""
from datetime import datetime
import json
import os
import select
import time


class NativeWaylandClipboard:
    """只接受原生 data-control 的选区事件；带代次写回并等实际所有权回执。"""
    def __init__(self, process):
        self.process = process
        self.buffer = bytearray()
        self.generation = None
        self.text = None
        self.pending = True
        self.remote = False
        self.request = 0
        self.receipt = None

    def read(self):
        if self.process.poll() is not None:
            self.text = None
            return None
        while select.select([self.process.stdout], [], [], 0)[0]:
            data = os.read(self.process.stdout.fileno(), 4096)
            if not data:
                self.text = None
                return None
            self.buffer.extend(data)
            if len(self.buffer) > 131072:
                self.buffer.clear()
                self.text = None
                self.pending = True
                return None
            while b"\n" in self.buffer:
                line, _, remaining = self.buffer.partition(b"\n")
                self.buffer[:] = remaining
                try:
                    event = json.loads(line)
                    generation = event["generation"]
                    text = event["text"]
                    if (not isinstance(generation, int) or generation < 0 or
                            (text is not None and not isinstance(text, str))):
                        raise ValueError()
                    self.generation = generation
                    self.text = text
                    self.pending = event.get("pending") is not False
                    self.remote = event.get("remote") is True
                    if "request" in event:
                        self.receipt = (event["request"], event.get("written") is True)
                except (ValueError, KeyError, TypeError):
                    self.text = None
                    self.pending = True
        return None if self.pending or self.buffer else self.text

    def write(self, text):
        if self.read() is None:
            return False
        self.request += 1
        action = {"request": self.request, "generation": self.generation, "text": text}
        self.process.stdin.write((json.dumps(action, ensure_ascii=False) + "\n").encode())
        self.process.stdin.flush()
        deadline = time.monotonic() + 1.5
        while time.monotonic() < deadline:
            current = self.read()
            if self.receipt is not None and self.receipt[0] == self.request:
                return self.receipt[1] and self.remote and current == text
            if self.process.poll() is not None:
                return False
            select.select([self.process.stdout], [], [], max(0, deadline - time.monotonic()))
        return False


class CloudReceivePolicy:
    """本机内容建立一次云端水位；写入成功才消费新修订。"""
    def __init__(self):
        self.local = None
        self.latest = None
        self.baseline = True

    def changed(self, text):
        if text != self.local:
            self.local = text
            self.baseline = True

    def consider(self, item, current):
        self.changed(current)
        if current is None:
            return False
        revision = None
        if item is not None:
            try:
                updated = item.get("updated_at")
                if not isinstance(updated, str):
                    return False
                revision = datetime.fromisoformat(updated.replace("Z", "+00:00"))
                if revision.utcoffset() is None:
                    return False
            except (ValueError, TypeError, KeyError):
                return False
        if self.baseline:
            if revision is not None and (self.latest is None or revision > self.latest):
                self.latest = revision
            self.baseline = False
            return False
        if revision is None or (self.latest is not None and revision <= self.latest):
            return False
        if item["text"] == current:
            self.latest = revision
            return False
        return True

    def received(self, item):
        self.latest = datetime.fromisoformat(item["updated_at"].replace("Z", "+00:00"))
        self.local = item["text"]
        self.baseline = False


class CloudAutoSync:
    def __init__(self, request):
        self.request = request
        self.binding = None
        self.observed = None
        self.observed_revision = None
        self.receive = CloudReceivePolicy()

    def reset(self):
        self.binding = None
        self.observed = None
        self.observed_revision = None
        self.receive = CloudReceivePolicy()

    def poll(self, read, write, receive_allowed=True, revision=lambda: None, remote=lambda: False):
        before = read()
        before_revision = revision()
        binding = self.request({"operation": "binding"})
        if not isinstance(binding, dict) or not binding.get("source"):
            self.reset()
            return
        same_binding = binding == self.binding
        changed = same_binding and (before != self.observed or before_revision != self.observed_revision)
        if not same_binding:
            self.receive = CloudReceivePolicy()
        self.binding = binding
        # 每次复制最多尝试一次；网络失败不把旧正文留给下一次登录。
        self.observed = before
        self.observed_revision = before_revision
        page = self.request({"operation": "list", "search": "", "expected_binding": binding})
        current = read()
        current_revision = revision()
        changed = changed or (same_binding and (current != before or current_revision != before_revision))
        self.observed = current
        self.observed_revision = current_revision
        if self.request({"operation": "binding"}) != binding:
            self.reset()
            return
        if not isinstance(page, dict) or page.get("enabled") is not True:
            return
        encryption = page.get("e2e")
        if encryption is not None and (not isinstance(encryption, dict) or encryption.get("status") == "pending"):
            return
        items = page.get("items")
        if not isinstance(items, list):
            return
        item = items[0] if items else None
        if item is not None and (not isinstance(item, dict) or
                                 not isinstance(item.get("text"), str)):
            return
        if item is not None and (not item["text"] or "\0" in item["text"] or
                                 len(item["text"].encode("utf-16-le")) > 8000):
            return
        self.receive.changed(before)
        if changed:
            # 同文本再次复制也重建水位；晚到的接收回执不作为新的本机上传。
            self.receive.baseline = True
        receive = self.receive.consider(item, current)
        if changed:
            if not remote() and current and (item is None or current != item["text"]):
                self.request({"operation": "add", "text": current, "expected_binding": binding})
            return
        if (receive_allowed and receive and item is not None and read() == current and
                revision() == current_revision):
            if write(item["text"]) and read() == item["text"]:
                self.receive.received(item)
                # 接收的所有权变化直接消费，历史监听或下轮轮询都不回传。
                self.observed = item["text"]
                self.observed_revision = revision()
