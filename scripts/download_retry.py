"""打开锁定的下载，重试 CDN 偶发的瞬时失败。

``fetch_*.py`` 从 GitHub Release 下载资源，它的 CDN 偶尔对单个请求回 5xx；这样一次回应曾经让整个发版构建失败（Release Linux run 37259338700，``msime-cantonese.db`` 上的 ``HTTP Error 500``）。5xx、429、超时和连接失败按 ``RETRY_DELAYS`` 等待后重试；其他 HTTP 状态（404、403……）立刻失败，再问一次也不会变。证书校验失败同样不重试：它说明对端证书不对，不是网络抖动。只重试打开响应这一步：调用方把响应体写进暂存文件，之后核对大小和 SHA-256，响应体中途断掉或校验和对不上照样被拒绝，不会重试。

GitHub 的 504 会成片出现：2026-11-05 iOS CI 下载 ``libonnxruntime-aarch64-apple-ios-sim.a.gz``，从 09:10:53 到 09:11:27 连续四次 ``HTTP Error 504: Gateway Time-out``，原来 2、5、15 秒共 22 秒的等待全部落在这段里。现在第一次重试前等 2 秒，之后每次翻倍、单次最多 60 秒，最多重试 7 次（重试之间合计等 182 秒）；再加 ``RETRY_BUDGET`` 封顶：从第一次请求算起，下一次重试会晚于这个时长才开始时就放弃，免得连接每次都挂满 ``timeout`` 时把作业拖上半小时。
"""
import ssl
import sys
import time
import urllib.error
import urllib.request

FIRST_DELAY = 2
MAX_DELAY = 60
MAX_RETRIES = 7
# 每次重试前等待的秒数：指数退避，单次不超过 MAX_DELAY。
RETRY_DELAYS = tuple(min(MAX_DELAY, FIRST_DELAY * 2**retry) for retry in range(MAX_RETRIES))
# 从第一次请求开始计时的总时长上限（秒），包括每次请求本身花的时间。
RETRY_BUDGET = 600


def transient(error: Exception) -> bool:
    if isinstance(error, urllib.error.HTTPError):
        return error.code == 429 or error.code >= 500
    if isinstance(error, urllib.error.URLError) and isinstance(error.reason, ssl.SSLCertVerificationError):
        return False
    return isinstance(error, (urllib.error.URLError, TimeoutError, ConnectionError))


def urlopen(url, timeout: float):
    """带重试的 ``urllib.request.urlopen``；``url`` 是 URL 字符串或 ``urllib.request.Request``。"""
    started = time.monotonic()
    for delay in (*RETRY_DELAYS, None):
        try:
            return urllib.request.urlopen(url, timeout=timeout)
        except (urllib.error.URLError, TimeoutError, ConnectionError) as error:
            if not transient(error):
                raise
            target = url.full_url if isinstance(url, urllib.request.Request) else url
            detail = ""
            if isinstance(error, urllib.error.HTTPError):
                # GitHub's release downloads redirect to release-assets.githubusercontent.com, so the hop that failed is named; GitHub Support can trace a failure by its X-GitHub-Request-Id.
                request_id = error.headers.get("X-GitHub-Request-Id") if error.headers else None
                detail = f" (from {error.url}" + (f", X-GitHub-Request-Id {request_id}" if request_id else "") + ")"
            if delay is not None and time.monotonic() - started + delay > RETRY_BUDGET:
                delay = None
            outcome = "giving up" if delay is None else f"retrying in {delay}s"
            print(f"  {target}: {type(error).__name__}: {error}{detail}; {outcome}", file=sys.stderr)
            if delay is None:
                raise
            if isinstance(error, urllib.error.HTTPError):
                error.close()
            time.sleep(delay)
