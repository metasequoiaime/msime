#!/usr/bin/env python3
"""Hand a freshly uploaded build to its testers.

`altool --upload-app` finishes when App Store Connect has the file, and that is all it does. The
build then sits undistributed until somebody opens the website, so every release before this one
uploaded a build its testers never saw: they stayed on whichever one a person last handed over.

Two audiences, reached differently. A merge into main goes to the internal group, which needs no
review and so reaches the people waiting on the change as soon as processing ends. A deliberate
release also goes to the external group, and that one Apple must review -- a queue worth one slot
per release rather than one per merge.
"""
import argparse
import json
import re
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

import jwt

API = "https://api.appstoreconnect.apple.com/v1"
# Apple rejects a token older than 20 minutes; processing can outlast one, so it is minted per call.
TOKEN_LIFETIME = 15 * 60


# TestFlight 的「What to Test」只显示纯文本，最多 4000 个字符；超出时 App Store Connect 直接拒绝整个请求。
WHATS_NEW_LIMIT = 4000
TRUNCATED = "……其余改动见下方链接。"
# App Store Connect 不接受的字符：2026-11-26 的 nightly 在 whatsNew 上报 `Text for whatsNew contains invalid characters:'[⚡, 🔧, 🐞, ✨, 🙏]'`，正是发布说明分组标题开头的 emoji。分组符号（含变体选择符、零宽连接符和键帽组合符）在正文里也一并去掉，TestFlight 只显示纯文字。
EMOJI = re.compile(
    "[\U0001F000-\U0001FAFF\u2600-\u27BF\u2B00-\u2BFF\uFE00-\uFE0F\u200D\u20E3]"
)


class Failure(RuntimeError):
    pass


def testflight_text(markdown: str, footer: str = "") -> str:
    """把 generate-release-notes.py 的 Markdown 发布说明改写成 TestFlight 能显示的纯文本。

    TestFlight 不渲染 Markdown，`###`、`**`、反引号会原样出现在测试者面前。超出长度时在整行处截断，页脚（完整改动的链接）始终保留。emoji 在 App Store Connect 那边是非法字符，长度也按去掉之后的文本算。
    """
    lines = []
    for line in markdown.strip().splitlines():
        line = re.sub(r"^#{1,6}\s+", "", line.rstrip())
        # 单独成行的粗体是分小组的领域名（「**输入**」），去掉星号后就混进正文了，改用方头括号标出来。
        line = re.sub(r"^\*\*([^*]+)\*\*(.*)$", lambda m: f"【{m[1]}】{m[2]}" if not m[2].startswith("：") else f"{m[1]}{m[2]}", line)
        line = line.replace("**", "").replace("`", "")
        line = EMOJI.sub("", line)
        # 组标题开头的 emoji 去掉后会剩一个前导空格；正文里 emoji 两边也常各留一个。
        line = re.sub(r"^ +", "", line)
        line = re.sub(r" {2,}", " ", line)
        if line or (lines and lines[-1]):
            lines.append(line)
    body = "\n".join(lines).strip()
    tail = f"\n\n{footer.strip()}" if footer.strip() else ""
    if utf16_length(body + tail) <= WHATS_NEW_LIMIT:
        return body + tail
    room = WHATS_NEW_LIMIT - utf16_length(f"\n{TRUNCATED}{tail}")
    kept: list[str] = []
    for line in body.splitlines():
        if utf16_length("\n".join([*kept, line])) > room:
            break
        kept.append(line)
    kept_text = "\n".join(kept).rstrip()
    return f"{kept_text}\n{TRUNCATED}{tail}"


def utf16_length(text: str) -> int:
    """App Store Connect 按 UTF-16 码元计长度；emoji 已在 testflight_text 里去掉，生僻字这类非 BMP 字符仍占两个，按 len() 算会超限。"""
    return len(text.encode("utf-16-le")) // 2


def token(key_id: str, issuer_id: str, key_path: Path) -> str:
    now = int(time.time())
    return jwt.encode(
        {"iss": issuer_id, "iat": now, "exp": now + TOKEN_LIFETIME, "aud": "appstoreconnect-v1"},
        key_path.read_text(),
        algorithm="ES256",
        headers={"kid": key_id, "typ": "JWT"},
    )


def request(method: str, path: str, auth: str, body: dict | None = None) -> dict:
    url = path if path.startswith("http") else f"{API}{path}"
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(url, data=data, method=method)
    req.add_header("Authorization", f"Bearer {auth}")
    if data is not None:
        req.add_header("Content-Type", "application/json")
    try:
        with urllib.request.urlopen(req, timeout=60) as response:
            payload = response.read()
            return json.loads(payload) if payload else {}
    except urllib.error.HTTPError as error:
        detail = error.read().decode(errors="replace")
        raise Failure(f"{method} {url} -> {error.code}\n{detail}") from None


def find_build(credentials: tuple, app_id: str, version: str, deadline: float) -> dict:
    """The build with this exact CFBundleVersion, waiting for it to appear first.

    A successful upload does not put the build in the API straight away -- altool returns as soon as
    Apple has the bytes, and the build shows up minutes later. Asking once and giving up turned a
    finished upload into "No build 1002.68.1 ... Most recent: 2, 1000.1.1, ...", which reads as a
    failed release and is not one: the package was already with Apple and only the distribution was
    missing, leaving the testers waiting while the run looked broken.
    """
    announced = False
    while True:
        page = request("GET", f"/builds?filter[app]={app_id}&filter[version]={version}&limit=10", token(*credentials))
        for build in page.get("data", []):
            if build["attributes"]["version"] == version:
                return build
        if time.time() >= deadline:
            break
        if not announced:
            print(f"waiting for build {version} to appear", flush=True)
            announced = True
        time.sleep(30)
    recent = request("GET", f"/builds?filter[app]={app_id}&sort=-uploadedDate&limit=5", token(*credentials))
    names = ", ".join(b["attributes"]["version"] for b in recent.get("data", []))
    raise Failure(f"No build {version} for app {app_id} by the processing deadline. Most recent: {names or 'none'}")


def await_processing(credentials: tuple, build_id: str, deadline: float) -> None:
    """Apple refuses to distribute a build it is still processing, and processing is minutes long."""
    seen = ""
    while time.time() < deadline:
        state = request("GET", f"/builds/{build_id}", token(*credentials))["data"]["attributes"]["processingState"]
        if state != seen:
            print(f"processing: {state}", flush=True)
            seen = state
        if state == "VALID":
            return
        if state in {"INVALID", "FAILED"}:
            raise Failure(f"Build {build_id} finished processing as {state}")
        time.sleep(30)
    raise Failure(f"Build {build_id} was still {seen or 'processing'} at the processing deadline")


def find_group(auth: str, app_id: str, name: str) -> tuple[str, bool]:
    """The group's id, and whether it is an internal one -- the two callers need both."""
    page = request("GET", f"/betaGroups?filter[app]={app_id}&limit=50", auth)
    for group in page.get("data", []):
        if group["attributes"]["name"] == name:
            return group["id"], bool(group["attributes"].get("isInternalGroup"))
    names = ", ".join(g["attributes"]["name"] for g in page.get("data", []))
    raise Failure(f"No beta group named {name!r}. Groups: {names or 'none'}")


def set_whats_new(auth: str, app_id: str, build_id: str, text: str) -> list[str]:
    """把这一版改了什么写进 build 的「What to Test」，测试者在 TestFlight 里看到的就是它。

    写进应用的主语言以及每一个已有的测试信息语言：TestFlight 按设备语言挑本地化，只写一种的话，其他语言的设备上还是空的。build 刚处理完时 Apple 可能已经替它建好了某些语言的条目，那些改（PATCH），没有的新建（POST）。
    """
    primary = request("GET", f"/apps/{app_id}?fields[apps]=primaryLocale", auth)["data"]["attributes"]["primaryLocale"]
    app_locales = request("GET", f"/apps/{app_id}/betaAppLocalizations?limit=50", auth)
    locales = [primary] + sorted({item["attributes"]["locale"] for item in app_locales.get("data", [])} - {primary})
    existing = {
        item["attributes"]["locale"]: item["id"]
        for item in request("GET", f"/builds/{build_id}/betaBuildLocalizations?limit=50", auth).get("data", [])
    }
    for locale in locales:
        if locale in existing:
            request("PATCH", f"/betaBuildLocalizations/{existing[locale]}", auth,
                    {"data": {"type": "betaBuildLocalizations", "id": existing[locale], "attributes": {"whatsNew": text}}})
        else:
            request("POST", "/betaBuildLocalizations", auth,
                    {"data": {"type": "betaBuildLocalizations", "attributes": {"locale": locale, "whatsNew": text},
                              "relationships": {"build": {"data": {"type": "builds", "id": build_id}}}}})
    return locales


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--app", required=True)
    parser.add_argument("--build-version", required=True, help="CFBundleVersion of the upload")
    parser.add_argument("--group", required=True, help="Beta group to distribute to")
    parser.add_argument("--submit-review", action="store_true",
                        help="Ask Apple to review the build, which external testing requires")
    parser.add_argument("--key-id", required=True)
    parser.add_argument("--issuer-id", required=True)
    parser.add_argument("--key-path", required=True, type=Path)
    parser.add_argument("--processing-timeout", type=int, default=45 * 60)
    parser.add_argument("--whats-new-file", type=Path,
                        help="Markdown release notes to show testers as the build's What to Test")
    parser.add_argument("--whats-new-footer", default="", help="Line kept at the end of What to Test, e.g. a compare link")
    arguments = parser.parse_args()

    credentials = (arguments.key_id, arguments.issuer_id, arguments.key_path)
    # 出现和处理完是同一段等待的两半,共用一个预算:上传之后先等它出现在 API 里,再等它处理成 VALID。
    # 等待可以远超 token 的 15 分钟有效期,所以两个等待函数拿凭据而不是 token,每次请求现签一个;只签一次的话,第 15 分钟以后的请求全部 401。
    deadline = time.time() + arguments.processing_timeout
    build = find_build(credentials, arguments.app, arguments.build_version, deadline)
    print(f"build {arguments.build_version} is {build['id']}", flush=True)

    await_processing(credentials, build["id"], deadline)

    auth = token(*credentials)
    # 先写「What to Test」再分发：外部组提交审核时 Apple 一并审这段文字，内部组处理完就已经拿到 build，越早写上测试者越早看到。写不进去不能拦下分发 —— 测试者拿不到 build 比拿到一个没写说明的 build 更糟 —— 但这一步最后要报红，否则说明又会像之前那样悄悄空着。
    whats_new_error = None
    if arguments.whats_new_file:
        try:
            text = testflight_text(arguments.whats_new_file.read_text(encoding="utf-8"), arguments.whats_new_footer)
            locales = set_whats_new(auth, arguments.app, build["id"], text)
            print(f"What to Test set for {', '.join(locales)} ({utf16_length(text)} UTF-16 units)", flush=True)
        except (Failure, OSError, KeyError) as error:
            whats_new_error = error
            print(f"::error::What to Test was not set: {error}", flush=True)
    status = 1 if whats_new_error else 0

    group, internal = find_group(auth, arguments.app, arguments.group)
    if internal:
        # 内部组自动拥有每一个 build,Apple 也不接受把 build 显式加进去:那个 POST 回 422
        # "Builds cannot be assigned to this internal group."。处理完成就等于内测已经拿到了,
        # 这一步不是可选的优化,是做了必错 —— 它让每一次合并到 main 的发布都染红,而内测其实是好的。
        print(f"internal group {arguments.group} already has every build", flush=True)
    else:
        request("POST", f"/betaGroups/{group}/relationships/builds", auth,
                {"data": [{"type": "builds", "id": build["id"]}]})
        print(f"added to beta group {arguments.group}", flush=True)

    # 内部组自己就能分发,只有外部测试要过 Apple 的审核。
    if not arguments.submit_review:
        return status

    # Already submitted is the state we want, not a failure: a rerun of a published release should
    # not turn red for finding its own work done.
    try:
        request("POST", "/betaAppReviewSubmissions", auth,
                {"data": {"type": "betaAppReviewSubmissions",
                          "relationships": {"build": {"data": {"type": "builds", "id": build["id"]}}}}})
    except Failure as error:
        if "ENTITY_ERROR" in str(error) or "already" in str(error).lower():
            print("already submitted for beta review", flush=True)
            return status
        raise
    print("submitted for beta review", flush=True)
    return status


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Failure as error:
        print(f"error: {error}", file=sys.stderr)
        sys.exit(1)
