"""云剪贴板的单进程同步状态；不保存正文队列。"""
from datetime import datetime


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
        self.receive = CloudReceivePolicy()

    def reset(self):
        self.binding = None
        self.observed = None
        self.receive = CloudReceivePolicy()

    def poll(self, read, write, receive_allowed=True):
        before = read()
        binding = self.request({"operation": "binding"})
        if not isinstance(binding, dict) or not binding.get("source"):
            self.reset()
            return
        same_binding = binding == self.binding
        changed = same_binding and before != self.observed
        if not same_binding:
            self.receive = CloudReceivePolicy()
        self.binding = binding
        # 每次复制最多尝试一次；网络失败不把旧正文留给下一次登录。
        self.observed = before
        page = self.request({"operation": "list", "search": "", "expected_binding": binding})
        current = read()
        changed = changed or (same_binding and current != before)
        self.observed = current
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
        receive = self.receive.consider(item, current)
        if changed:
            if current and (item is None or current != item["text"]):
                self.request({"operation": "add", "text": current, "expected_binding": binding})
            return
        if receive_allowed and receive and item is not None and read() == current:
            if write(item["text"]) and read() == item["text"]:
                self.receive.received(item)
                # 接收的所有权变化直接消费，历史监听或下轮轮询都不回传。
                self.observed = item["text"]
