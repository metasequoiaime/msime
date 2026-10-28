#!/usr/bin/env python3
"""只执行生产代码为合成拼音 nihao 构造的百度 HTTPS 描述符。"""
import json
import sys
import urllib.request
from pathlib import Path

class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None

def main():
    descriptor = json.loads(Path(sys.argv[1]).read_bytes())
    expected = "https://olimenew.baidu.com/py?input=nihao&inputtype=py&resultcoding=utf-8"
    if descriptor["url"] != expected or descriptor["method"] != "GET":
        raise ValueError("unexpected synthetic probe descriptor")
    request = urllib.request.Request(descriptor["url"], headers=descriptor["headers"])
    with urllib.request.build_opener(NoRedirect()).open(request, timeout=descriptor["timeout_ms"] / 1000) as response:
        body = response.read(descriptor["max_response_bytes"] + 1)
    if len(body) > descriptor["max_response_bytes"]:
        raise ValueError("response exceeds production limit")
    Path(sys.argv[2]).write_bytes(body)
    print(json.dumps({"live_request":"baidu:nihao", "response_bytes":len(body)}))

if __name__ == "__main__":
    main()
