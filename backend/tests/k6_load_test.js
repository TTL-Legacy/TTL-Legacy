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
  },
  thresholds: {
    http_req_duration: ['p(95)<500', 'p(99)<1000'],
    http_req_failed: ['rate<0.1'],
    'http_req_duration{scenario:concurrent_checkins}': ['p(99)<1000'],
    'http_req_failed{scenario:concurrent_checkins}': ['rate<0.01'],
  },
};

const BASE_URL = __ENV.BASE_URL || 'http://localhost:3000';
const VAULT_ID = __ENV.VAULT_ID || 'test-vault-123';
const VAULT_ID_PREFIX = __ENV.VAULT_ID_PREFIX || 'test-vault';
const VAULT_COUNT = Number(__ENV.VAULT_COUNT || 1000);

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

  return summary;
}
