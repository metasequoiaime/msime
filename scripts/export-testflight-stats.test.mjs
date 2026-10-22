import assert from 'node:assert/strict';
import { test } from 'node:test';
import { generateKeyPairSync, verify } from 'node:crypto';
import { appleToken, applePages, buildInstalls, collectStats, dispatchStats } from './export-testflight-stats.mjs';

const pair = generateKeyPairSync('ec', { namedCurve: 'prime256v1' });
const credentials = { keyId: 'SYNTHETIC', issuerId: 'synthetic-issuer', privateKey: pair.privateKey.export({ type: 'pkcs8', format: 'pem' }) };
const point = (count, start = '2026-10-01', end = '2026-10-21') => ({ start, end, values: { installCount: count } });
const metrics = (points = [point(5)]) => [{ type: 'betaBuildUsages', dataPoints: points }];
const build = id => ({ type: 'builds', id, attributes: { processingState: 'VALID' } });
const answer = (data, next = null) => Response.json({ data, links: { next } });

test('JWT uses verifiable P-256 ES256 and expires within the Apple limit', () => {
  const [header, payload, signature] = appleToken(credentials, 1000000).split('.');
  assert.deepEqual(JSON.parse(Buffer.from(header, 'base64url')), { alg: 'ES256', kid: 'SYNTHETIC', typ: 'JWT' });
  assert.deepEqual(JSON.parse(Buffer.from(payload, 'base64url')), { iss: 'synthetic-issuer', iat: 1000, exp: 1900, aud: 'appstoreconnect-v1' });
  assert.ok(verify('sha256', Buffer.from(`${header}.${payload}`), { key: pair.publicKey, dsaEncoding: 'ieee-p1363' }, Buffer.from(signature, 'base64url')));
});

test('complete build pagination deduplicates builds and collects only aggregate installs', async () => {
  const urls = [];
  const request = async (url, init) => {
    urls.push(url);
    assert.equal(init.redirect, 'error');
    assert.ok(init.headers.Authorization.startsWith('Bearer '));
    const parsed = new URL(url);
    if (parsed.pathname === '/v1/builds') {
      assert.equal(parsed.searchParams.get('filter[app]'), '123');
      assert.equal(parsed.searchParams.get('filter[preReleaseVersion.platform]'), 'IOS');
      return parsed.searchParams.has('cursor') ? answer([build('a'), build('b')]) : answer([build('a')], `${url}&cursor=next`);
    }
    assert.ok(!url.includes('betaTester'));
    assert.equal(parsed.searchParams.has('limit'), false, 'the live build usage API rejects limit; pagination follows Apple links');
    return answer(metrics([point(url.includes('/a/') ? 5 : 0)]));
  };
  assert.deepEqual(await collectStats('123', credentials, request, new Date('2026-10-21T10:00:00Z')), {
    schemaVersion: 1, generatedAt: '2026-10-21T10:00:00.000Z', installs: 5, buildCount: 2, periodStart: '2026-10-01', periodEnd: '2026-10-21',
  });
  assert.equal(urls.length, 4);
});

test('non-overlapping metrics buckets sum but absent, grouped or invalid counts fail', () => {
  assert.equal(buildInstalls(metrics([point(2, '2026-10-01', '2026-10-10'), point(3, '2026-10-11', '2026-10-21')])).installs, 5);
  assert.equal(buildInstalls(metrics([point(0)])).installs, 0);
  for (const dimensions of [null, {}]) assert.equal(buildInstalls([{ ...metrics()[0], dimensions }]).installs, 5);
  for (const rows of [[], metrics([]), metrics([point(null)]), metrics([point(-1)]), metrics([point('1')]), metrics([point(1.2)]), metrics([point(Number.MAX_SAFE_INTEGER + 1)]), metrics([point(1), point(1)]), metrics([point(1, '2026-02-30')]), [{ ...metrics()[0], dimensions: { testers: {} } }]]) {
    assert.throws(() => buildInstalls(rows));
  }
});

test('failed later pages or metrics never produce a partial aggregate', async () => {
  let calls = 0;
  await assert.rejects(collectStats('123', credentials, async url => {
    calls++;
    return calls === 1 ? answer([build('a')], `${url}&cursor=next`) : new Response(null, { status: 503 });
  }), /HTTP 503/);
  await assert.rejects(collectStats('123', credentials, async url => new URL(url).pathname === '/v1/builds' ? answer([build('a'), build('b')]) : url.includes('/a/') ? answer(metrics()) : new Response(null, { status: 403 })), /HTTP 403/);
  await assert.rejects(collectStats('123', credentials, async () => answer([])), /No available/);
});

test('pagination rejects redirects to other hosts, other Apple resources and cycles before sending credentials', async () => {
  for (const next of ['https://outside.example/v1/builds', 'https://api.appstoreconnect.apple.com/v1/betaTesters', 'https://api.appstoreconnect.apple.com/v1/builds']) {
    let calls = 0;
    await assert.rejects(applePages('/v1/builds', credentials, async () => { calls++; return answer([], next); }), /pagination/);
    assert.equal(calls, 1);
  }
});

test('site dispatch strips raw fields and refuses unsuccessful delivery', async () => {
  const stats = { schemaVersion: 1, generatedAt: '2026-10-21T10:00:00.000Z', installs: 5, buildCount: 2, periodStart: '2026-10-01', periodEnd: '2026-10-21' };
  let body;
  await dispatchStats({ ...stats, raw: { syntheticPrivateField: true } }, 'synthetic-dispatch-token', async (url, init) => {
    assert.equal(url, 'https://api.github.com/repos/metasequoiaime/msime-web/dispatches');
    body = JSON.parse(init.body);
    return new Response(null, { status: 204 });
  });
  assert.deepEqual(body, { event_type: 'testflight-stats-v1', client_payload: stats });
  await assert.rejects(dispatchStats(stats, 'synthetic', async () => new Response(null, { status: 403 })), /HTTP 403/);
});


test('Apple failures expose only endpoint, code and known parameter names', async () => {
  const response = () => Response.json({ errors: [{ code: 'PARAMETER_ERROR.INVALID', source: { parameter: 'filter[app]' }, detail: 'synthetic-private-detail' }, { code: 'synthetic-private-code', source: { parameter: 'synthetic-private-parameter' } }] }, { status: 400 });
  await assert.rejects(applePages('/v1/builds?filter[app]=123', credentials, response), error => {
    assert.match(error.message, /HTTP 400; endpoint=build list; codes=PARAMETER_ERROR.INVALID; parameters=filter\[app\]/);
    assert.doesNotMatch(error.message, /synthetic-private|123|Bearer/);
    return true;
  });
  await assert.rejects(applePages('/v1/builds/synthetic-id/metrics/betaBuildUsages', credentials, async () => new Response('synthetic-private-non-json', { status: 400 })), error => {
    assert.match(error.message, /endpoint=build usage/);
    assert.doesNotMatch(error.message, /synthetic/);
    return true;
  });
});
