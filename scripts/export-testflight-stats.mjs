import { createPrivateKey, sign } from 'node:crypto';
import { pathToFileURL } from 'node:url';

const APPLE = 'https://api.appstoreconnect.apple.com';
export class StatsError extends Error {}
const check = (condition, message) => { if (!condition) throw new StatsError(message); };
const date = value => typeof value === 'string' && /^\d{4}-\d{2}-\d{2}$/.test(value) && Number.isFinite(Date.parse(value)) && new Date(value).toISOString().slice(0, 10) === value;

/** 每次请求重新签发短期令牌，私钥只在当前进程内解码，不写入文件。 */
export function appleToken({ keyId, issuerId, privateKey }, now = Date.now()) {
  const iat = Math.floor(now / 1000);
  const encode = value => Buffer.from(JSON.stringify(value)).toString('base64url');
  const body = `${encode({ alg: 'ES256', kid: keyId, typ: 'JWT' })}.${encode({ iss: issuerId, iat, exp: iat + 900, aud: 'appstoreconnect-v1' })}`;
  const key = createPrivateKey(privateKey);
  check(key.asymmetricKeyType === 'ec' && key.asymmetricKeyDetails?.namedCurve === 'prime256v1', 'Apple key must use P-256');
  return `${body}.${sign('sha256', Buffer.from(body), { key, dsaEncoding: 'ieee-p1363' }).toString('base64url')}`;
}

/** 分页只访问原端点；禁止将 Apple 授权转发到其他主机或资料接口。 */
export async function applePages(path, credentials, request = fetch) {
  const endpoint = new URL(path, APPLE);
  let next = endpoint.href;
  const seen = new Set();
  const rows = [];
  while (next) {
    const url = new URL(next);
    check(url.origin === APPLE && !url.username && !url.password && !url.hash && url.pathname === endpoint.pathname, 'Invalid Apple pagination URL');
    check(!seen.has(url.href) && seen.size < 1000, 'Invalid Apple pagination cycle');
    seen.add(url.href);
    const response = await request(url.href, { headers: { Authorization: `Bearer ${appleToken(credentials)}`, Accept: 'application/json' }, signal: AbortSignal.timeout(30000), redirect: 'error' });
    check(response.ok, `Apple metrics request failed: HTTP ${response.status}`);
    const page = await response.json();
    check(Array.isArray(page.data), 'Invalid Apple metrics response');
    rows.push(...page.data);
    next = page.links?.next ?? null;
    check(next === null || typeof next === 'string', 'Invalid Apple pagination link');
  }
  return rows;
}

/** 构建接口不按测试者分组；拒绝缺失计数和重叠时间桶，避免静默发布部分数值。 */
export function buildInstalls(rows) {
  check(rows.length === 1 && rows[0].type === 'betaBuildUsages', 'Unavailable Apple build metrics');
  const dimensions = rows[0].dimensions;
  check(dimensions == null || (typeof dimensions === 'object' && !Array.isArray(dimensions) && Object.keys(dimensions).length === 0), 'Grouped Apple build metrics');
  const points = rows[0].dataPoints;
  check(Array.isArray(points) && points.length > 0, 'Unavailable Apple build metrics');
  const sorted = [...points].sort((a, b) => String(a.start).localeCompare(String(b.start)));
  let installs = 0;
  for (const [index, point] of sorted.entries()) {
    check(date(point.start) && date(point.end) && point.start <= point.end, 'Invalid Apple metrics interval');
    check(index === 0 || sorted[index - 1].end < point.start, 'Overlapping Apple metrics intervals');
    const count = point.values?.installCount;
    check(Number.isSafeInteger(count) && count >= 0, 'Missing or invalid Apple install count');
    installs += count;
    check(Number.isSafeInteger(installs), 'Apple install count overflow');
  }
  return { installs, periodStart: sorted[0].start, periodEnd: sorted.at(-1).end };
}

export async function collectStats(appId, credentials, request = fetch, now = new Date()) {
  check(/^\d+$/.test(appId), 'Missing or invalid Apple app ID');
  const query = new URLSearchParams({ 'filter[app]': appId, 'filter[processingState]': 'VALID', 'filter[preReleaseVersion.platform]': 'IOS', 'fields[builds]': 'processingState', limit: '200' });
  const builds = await applePages(`/v1/builds?${query}`, credentials, request);
  const ids = new Set();
  let installs = 0;
  let periodStart = null;
  let periodEnd = null;
  for (const build of builds) {
    check(build.type === 'builds' && typeof build.id === 'string' && /^[a-z0-9-]+$/i.test(build.id) && build.attributes?.processingState === 'VALID', 'Invalid Apple build response');
    if (ids.has(build.id)) continue;
    ids.add(build.id);
    const stats = buildInstalls(await applePages(`/v1/builds/${build.id}/metrics/betaBuildUsages?limit=200`, credentials, request));
    installs += stats.installs;
    check(Number.isSafeInteger(installs), 'Apple install count overflow');
    periodStart = periodStart === null || stats.periodStart < periodStart ? stats.periodStart : periodStart;
    periodEnd = periodEnd === null || stats.periodEnd > periodEnd ? stats.periodEnd : periodEnd;
  }
  check(ids.size > 0, 'No available Apple build metrics');
  return { schemaVersion: 1, generatedAt: now.toISOString(), installs, buildCount: ids.size, periodStart, periodEnd };
}

/** 只发送明确列出的公开汇总字段，原始 Apple 响应不会进入官网事件或日志。 */
export async function dispatchStats(stats, githubToken, request = fetch) {
  check(Boolean(githubToken), 'Missing site dispatch credential');
  const { schemaVersion, generatedAt, installs, buildCount, periodStart, periodEnd } = stats;
  const response = await request('https://api.github.com/repos/metasequoiaime/msime-web/dispatches', {
    method: 'POST',
    headers: { Authorization: `Bearer ${githubToken}`, Accept: 'application/vnd.github+json', 'Content-Type': 'application/json', 'X-GitHub-Api-Version': '2022-11-28' },
    body: JSON.stringify({ event_type: 'testflight-stats-v1', client_payload: { schemaVersion, generatedAt, installs, buildCount, periodStart, periodEnd } }),
    signal: AbortSignal.timeout(30000), redirect: 'error',
  });
  check(response.status === 204, `Site stats dispatch failed: HTTP ${response.status}`);
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    const env = process.env;
    check(Boolean(env.IOS_API_KEY_BASE64 && env.IOS_API_KEY_ID && env.IOS_API_KEY_ISSUER_ID), 'Missing Apple metrics credentials');
    const credentials = { keyId: env.IOS_API_KEY_ID, issuerId: env.IOS_API_KEY_ISSUER_ID, privateKey: Buffer.from(env.IOS_API_KEY_BASE64, 'base64').toString('utf8') };
    const stats = await collectStats(env.IOS_APP_ID ?? '', credentials);
    await dispatchStats(stats, env.GH_TOKEN);
    console.log(`TestFlight stats sent: ${stats.installs} installs, ${stats.buildCount} builds`);
  } catch (error) {
    console.error(error instanceof StatsError ? error.message : 'TestFlight stats sync failed; previous site snapshot is preserved');
    process.exitCode = 1;
  }
}
