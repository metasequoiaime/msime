#!/usr/bin/env python3
"""从安装好的 Linux 设置入口验证首次配置下载清单，不准备或下载词库。"""
import base64
import json
import os
from pathlib import Path
import subprocess
import sys
import time
import urllib.error
import urllib.request

prefix = Path(sys.argv[1]).resolve()
evidence = Path(sys.argv[2]).resolve()
evidence.mkdir(parents=True, exist_ok=True)
launcher = prefix / "bin/msime-linux-settings"
setup = prefix / "bin/msime-linux-setup"
expected = json.loads(subprocess.check_output([str(setup), "--list-downloads", "--json"]))
(evidence / "downloads.json").write_text(json.dumps(expected, ensure_ascii=False, indent=2))
state = Path(os.environ["MSIME_CLIENT_HOST_OPTIONS"]).parent
assert not state.exists(), "清单读取不得创建输入法状态目录"
assert not Path(expected["directory"]).exists(), "清单读取不得创建词库目录"

def request(method, path, payload=None):
    data = None if payload is None else json.dumps(payload).encode()
    req = urllib.request.Request("http://127.0.0.1:4444" + path, data=data, method=method,
                                 headers={"Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=90) as response:
            result = json.load(response)
    except urllib.error.HTTPError as error:
        raise RuntimeError(error.read().decode()) from error
    if isinstance(result.get("value"), dict) and "error" in result["value"]:
        raise RuntimeError(result)
    return result.get("value")

def wait_for(callback, seconds=45):
    deadline = time.monotonic() + seconds
    last = None
    while time.monotonic() < deadline:
        try:
            result = callback()
            if result:
                return result
        except (OSError, RuntimeError, KeyError, subprocess.SubprocessError) as error:
            last = error
        time.sleep(0.25)
    raise AssertionError(f"等待界面超时：{last}")

driver_log = (evidence / "driver.log").open("w")
driver = subprocess.Popen(["tauri-driver", "--port", "4444"], stdout=driver_log, stderr=driver_log)
session = None
try:
    wait_for(lambda: request("GET", "/status"))
    value = request("POST", "/session", {"capabilities": {"alwaysMatch": {
        "browserName": "wry", "tauri:options": {"application": str(launcher)}
    }}})
    session = value["sessionId"]
    root = "/session/" + session

    def evaluate(script, args=None):
        return request("POST", root + "/execute/sync", {"script": script, "args": args or []})

    def body():
        return evaluate("return document.body.innerText")

    def click(label, index=0):
        elements = evaluate("return [...document.querySelectorAll('button')].filter(e=>e.textContent.trim()===arguments[0])", [label])
        element = elements[index]
        evaluate("arguments[0].scrollIntoView({block:'center'}); return true", [element])
        request("POST", root + "/element/" + element["element-6066-11e4-a52e-4f735466cecf"] + "/click", {})

    def screenshot(name):
        (evidence / name).write_bytes(base64.b64decode(request("GET", root + "/screenshot")))

    wait_for(lambda: "手动下载词库" in body())
    assert "准备词库和配置" in body()
    screenshot("first-run.png")
    click("手动下载词库")
    wait_for(lambda: "复制保存目录" in body())
    text = body()
    assert expected["directory"] in text
    for artifact in expected["artifacts"]:
        assert artifact["name"] in text and artifact["sha256"] in text
        assert str(artifact["size"]) in text.replace(",", "")
        if artifact["url"]:
            assert artifact["url"] in text
        if artifact["mirrorUrl"]:
            assert artifact["mirrorUrl"] in text

    def copy_and_check(label, value, index=0):
        click(label, index)
        wait_for(lambda: subprocess.check_output(["xclip", "-selection", "clipboard", "-o"], timeout=5).decode() == value)

    copy_and_check("复制保存目录", expected["directory"])
    copy_and_check("复制全部原地址", "\n".join(a["url"] for a in expected["artifacts"] if a["url"]))
    original_index = mirror_index = 0
    for artifact in expected["artifacts"]:
        if artifact["url"]:
            copy_and_check("复制原地址", artifact["url"], original_index)
            original_index += 1
            if artifact["mirrorUrl"]:
                copy_and_check("复制国内镜像", artifact["mirrorUrl"], mirror_index)
                mirror_index += 1
    evaluate("document.querySelector('h2').scrollIntoView(); return true")
    screenshot("manual-downloads.png")
    click("手动下载词库")
    assert "复制保存目录" not in body()
    assert evaluate("return [...document.querySelectorAll('button')].some(e=>e.textContent.trim()==='开始配置'&&!e.disabled)")
    click("手动下载词库")
    wait_for(lambda: "复制保存目录" in body())
    assert not state.exists() and not Path(expected["directory"]).exists()
    geometry = evaluate("return {width:innerWidth,height:innerHeight,scrollHeight:document.documentElement.scrollHeight}")
    (evidence / "result.json").write_text(json.dumps({"result": "passed", "entry": str(launcher),
        "artifacts": len(expected["artifacts"]), "clipboardChecks": 2 + original_index + mirror_index,
        "geometry": geometry, "stateCreated": False, "dictionaryDirectoryCreated": False}, indent=2))
    print("首次配置清单显示、系统剪贴板、收起重开及只读目录断言全部通过")
finally:
    if session:
        try:
            screenshot("last-state.png")
            (evidence / "last-state.txt").write_text(body())
        except Exception:
            pass
        try:
            request("DELETE", "/session/" + session)
        except Exception:
            pass
    driver.terminate()
    try:
        driver.wait(timeout=10)
    except subprocess.TimeoutExpired:
        driver.kill()
        driver.wait()
    driver_log.close()
