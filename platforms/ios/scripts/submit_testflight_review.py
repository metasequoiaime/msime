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
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

import jwt

API = "https://api.appstoreconnect.apple.com/v1"
# Apple rejects a token older than 20 minutes; processing can outlast one, so it is minted per call.
TOKEN_LIFETIME = 15 * 60


class Failure(RuntimeError):
    pass


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


def find_build(auth: str, app_id: str, version: str, timeout: int) -> dict:
    """The build with this exact CFBundleVersion, waiting for it to appear first.

    A successful upload does not put the build in the API straight away -- altool returns as soon as
    Apple has the bytes, and the build shows up minutes later. Asking once and giving up turned a
    finished upload into "No build 1002.68.1 ... Most recent: 2, 1000.1.1, ...", which reads as a
    failed release and is not one: the package was already with Apple and only the distribution was
    missing, leaving the testers waiting while the run looked broken.
    """
    deadline = time.time() + timeout
    announced = False
    while True:
        page = request("GET", f"/builds?filter[app]={app_id}&filter[version]={version}&limit=10", auth)
        for build in page.get("data", []):
            if build["attributes"]["version"] == version:
                return build
        if time.time() >= deadline:
            break
        if not announced:
            print(f"waiting for build {version} to appear", flush=True)
            announced = True
        time.sleep(30)
    recent = request("GET", f"/builds?filter[app]={app_id}&sort=-uploadedDate&limit=5", auth)
    names = ", ".join(b["attributes"]["version"] for b in recent.get("data", []))
    raise Failure(f"No build {version} for app {app_id} after {timeout}s. Most recent: {names or 'none'}")


def await_processing(auth: str, build_id: str, timeout: int) -> None:
    """Apple refuses to distribute a build it is still processing, and processing is minutes long."""
    deadline = time.time() + timeout
    seen = ""
    while time.time() < deadline:
        state = request("GET", f"/builds/{build_id}", auth)["data"]["attributes"]["processingState"]
        if state != seen:
            print(f"processing: {state}", flush=True)
            seen = state
        if state == "VALID":
            return
        if state in {"INVALID", "FAILED"}:
            raise Failure(f"Build {build_id} finished processing as {state}")
        time.sleep(30)
    raise Failure(f"Build {build_id} was still {seen or 'processing'} after {timeout}s")


def find_group(auth: str, app_id: str, name: str) -> tuple[str, bool]:
    """The group's id, and whether it is an internal one -- the two callers need both."""
    page = request("GET", f"/betaGroups?filter[app]={app_id}&limit=50", auth)
    for group in page.get("data", []):
        if group["attributes"]["name"] == name:
            return group["id"], bool(group["attributes"].get("isInternalGroup"))
    names = ", ".join(g["attributes"]["name"] for g in page.get("data", []))
    raise Failure(f"No beta group named {name!r}. Groups: {names or 'none'}")


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
    arguments = parser.parse_args()

    credentials = (arguments.key_id, arguments.issuer_id, arguments.key_path)
    auth = token(*credentials)
    # 出现和处理完是同一段等待的两半,共用一个预算:上传之后先等它出现在 API 里,再等它处理成 VALID。
    build = find_build(auth, arguments.app, arguments.build_version, arguments.processing_timeout)
    print(f"build {arguments.build_version} is {build['id']}", flush=True)

    await_processing(token(*credentials), build["id"], arguments.processing_timeout)

    auth = token(*credentials)
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
        return 0

    # Already submitted is the state we want, not a failure: a rerun of a published release should
    # not turn red for finding its own work done.
    try:
        request("POST", "/betaAppReviewSubmissions", auth,
                {"data": {"type": "betaAppReviewSubmissions",
                          "relationships": {"build": {"data": {"type": "builds", "id": build["id"]}}}}})
    except Failure as error:
        if "ENTITY_ERROR" in str(error) or "already" in str(error).lower():
            print("already submitted for beta review", flush=True)
            return 0
        raise
    print("submitted for beta review", flush=True)
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Failure as error:
        print(f"error: {error}", file=sys.stderr)
        sys.exit(1)
