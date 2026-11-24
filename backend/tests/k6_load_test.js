import http from 'k6/http';
import exec from 'k6/execution';
import { check, sleep } from 'k6';

export const options = {
  scenarios: {
    default: {
      executor: 'ramping-vus',
      stages: [
        { duration: '2m', target: 100 },
        { duration: '5m', target: 100 },
        { duration: '2m', target: 0 },
      ],
      exec: 'default',
    },
    concurrent_checkins: {
      // 1000 VUs each run exactly one check-in, so all 1000 requests hit the API in parallel
      executor: 'shared-iterations',
      vus: 1000,
      iterations: 1000,
      maxDuration: '5m',
      exec: 'concurrentCheckIns',
    },
    // #2080: load test for GET /vaults/{id}/history under 10,000 historical entries.
    // The vault is pre-seeded with 10,000 check-in records before this scenario runs
    // (see the HISTORY_VAULT_ID env var and the seed script in backend/tests/seed_history.sh).
    check_in_history: {
      executor: 'constant-vus',
      vus: 100,
      duration: '3m',
      startTime: '9m', // start after the default scenario finishes
      exec: 'checkInHistory',
    },
  },
  thresholds: {
    http_req_duration: ['p(95)<500', 'p(99)<1000'],
    http_req_failed: ['rate<0.1'],
    'http_req_duration{scenario:concurrent_checkins}': ['p(99)<1000'],
    'http_req_failed{scenario:concurrent_checkins}': ['rate<0.01'],
    // #2080 assertions: p95 < 500 ms and no 5xx errors under 100 VUs against a
    // vault with 10,000 existing check-in records.
    'http_req_duration{scenario:check_in_history}': ['p(95)<500'],
    'http_req_failed{scenario:check_in_history}': ['rate<0.01'],
  },
};

const BASE_URL = __ENV.BASE_URL || 'http://localhost:3000';
const VAULT_ID = __ENV.VAULT_ID || 'test-vault-123';
const VAULT_ID_PREFIX = __ENV.VAULT_ID_PREFIX || 'test-vault';
const VAULT_COUNT = Number(__ENV.VAULT_COUNT || 1000);
// #2080: vault pre-seeded with 10,000 check-in records. Override with
// HISTORY_VAULT_ID when targeting a specific pre-seeded vault.
const HISTORY_VAULT_ID = __ENV.HISTORY_VAULT_ID || 'history-load-test-vault';

export function concurrentCheckIns() {
  const vaultId = `${VAULT_ID_PREFIX}-${exec.scenario.iterationInTest % VAULT_COUNT}`;
  const payload = JSON.stringify({
    signedPayload: 'test-payload-concurrent-' + exec.scenario.iterationInTest,
  });

  const params = {
    headers: {
      'Content-Type': 'application/json',
    },
  };

  const res = http.post(`${BASE_URL}/api/vaults/${vaultId}/check-in`, payload, params);

  check(res, {
    'concurrent status is 200': (r) => r.status === 200,
    'concurrent response has vault_id': (r) => r.json('vault_id') !== undefined,
    'concurrent response has checked_in_at': (r) => r.json('checked_in_at') !== undefined,
  });
}

/**
 * #2080 — GET /vaults/{id}/history load test.
 *
 * Hits the history endpoint against a vault pre-seeded with 10,000 check-in
 * records. Each VU paginates through pages to exercise both the first page
 * (cold path) and deeper pages (index scan path). The scenario thresholds
 * above enforce p95 < 500 ms and < 1 % error rate.
 *
 * Pre-seeding: run `backend/tests/seed_history.sh` (or the equivalent SQL
 * migration) before executing this scenario. See LOAD_TEST_README.md for
 * the full setup procedure.
 */
export function checkInHistory() {
  // Rotate through the first five pages so each VU exercises multiple offsets.
  const page = (exec.scenario.iterationInTest % 5) + 1;
  const limit = 100;
  const url = `${BASE_URL}/api/vaults/${HISTORY_VAULT_ID}/history?page=${page}&limit=${limit}`;

  const res = http.get(url, {
    tags: { endpoint: 'check_in_history' },
  });

  check(res, {
    'history status is 200': (r) => r.status === 200,
    'history response is not 5xx': (r) => r.status < 500,
    'history response time < 500ms': (r) => r.timings.duration < 500,
    'history response has items array': (r) => {
      try {
        const body = r.json();
        return Array.isArray(body) || (body && Array.isArray(body.items));
      } catch {
        return false;
      }
    },
  });

  sleep(0.5);
}

export default function () {
  const payload = JSON.stringify({
    signedPayload: 'test-payload-' + __VU + '-' + __ITER__,
  });

  const params = {
    headers: {
      'Content-Type': 'application/json',
    },
  };

  const res = http.post(`${BASE_URL}/api/vaults/${VAULT_ID}/check-in`, payload, params);

  check(res, {
    'status is 200': (r) => r.status === 200,
    'response has vault_id': (r) => r.json('vault_id') !== undefined,
    'response has checked_in_at': (r) => r.json('checked_in_at') !== undefined,
    'response time < 500ms': (r) => r.timings.duration < 500,
  });

  sleep(1);
}

export function handleSummary(data) {
  return {
    '/tmp/load-test-results.json': JSON.stringify(data, null, 2),
    stdout: textSummary(data, { indent: ' ', enableColors: true }),
  };
}

function textSummary(data, options) {
  const indent = options.indent || '';
  let summary = 'Load Test Summary:\n';

  summary += indent + `✓ Total requests: ${data.metrics.http_reqs.value}\n`;
  summary += indent + `✓ Failed requests: ${data.metrics.http_req_failed.value}\n`;
  summary += indent + `✓ Request rate: ${data.metrics.http_reqs.value / (data.state.testRunDurationMs / 1000)} req/s\n`;

  if (data.metrics.http_req_duration) {
    const duration = data.metrics.http_req_duration;
    summary += indent + `✓ Response time (avg): ${duration.value.toFixed(2)}ms\n`;
  }

  const concurrent = data.metrics['http_req_duration{scenario:concurrent_checkins}'];
  const concurrentFailed = data.metrics['http_req_failed{scenario:concurrent_checkins}'];
  if (concurrent) {
    summary += indent + `✓ Concurrent check-ins P99: ${concurrent.values['p(99)'].toFixed(2)}ms\n`;
  }
  if (concurrentFailed) {
    summary += indent + `✓ Concurrent check-ins error rate: ${(concurrentFailed.values.rate * 100).toFixed(2)}%\n`;
  }

  const history = data.metrics['http_req_duration{scenario:check_in_history}'];
  const historyFailed = data.metrics['http_req_failed{scenario:check_in_history}'];
  if (history) {
    const p95 = history.values['p(95)'];
    const passed = p95 < 500;
    summary += indent + `${passed ? '✓' : '✗'} History (10k records) P95: ${p95.toFixed(2)}ms (threshold <500ms)\n`;
  }
  if (historyFailed) {
    const rate = historyFailed.values.rate;
    const passed = rate < 0.01;
    summary += indent + `${passed ? '✓' : '✗'} History error rate: ${(rate * 100).toFixed(2)}% (threshold <1%)\n`;
  }

  return summary;
}
