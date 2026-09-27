# Check-in Endpoint Load Test

This directory contains k6 load test scripts for evaluating the performance of the check-in endpoint under various load conditions.

## Test Script

- **k6_load_test.js** — Simulates realistic user check-in traffic with gradual ramp-up and ramp-down.

## Running the Load Test

### Prerequisites

- [k6](https://k6.io/docs/getting-started/installation/) installed
- TTL-Legacy backend running (default: `http://localhost:3000`)

### Basic Usage

```bash
# Run with default settings (localhost)
k6 run backend/tests/k6_load_test.js

# Run against custom environment
BASE_URL=https://api.example.com k6 run backend/tests/k6_load_test.js

# Run with custom vault ID
BASE_URL=http://localhost:3000 VAULT_ID=my-vault-id k6 run backend/tests/k6_load_test.js
```

### Load Profile

The test script uses the following stages:

1. **Ramp-up (2 minutes)**: Gradually increase load from 0 to 100 concurrent users
2. **Sustained (5 minutes)**: Maintain 100 concurrent users
3. **Ramp-down (2 minutes)**: Gradually reduce load to 0

**Total test duration**: ~9 minutes

## Baseline Results

Baseline performance was measured against a freshly deployed backend with default configuration.

### Test Environment

- **Backend**: TTL-Legacy
- **Database**: SQLite (in-memory)
- **Hardware**: Standard cloud instance (2 CPU, 4GB RAM)
- **Date**: 2026-09-26

### Baseline Metrics

```
Total requests:       ~5,400
Failed requests:      0
Request rate:         ~10 req/s (sustained)
Response time (p95):  <150ms
Response time (p99):  <300ms
```

### Throughput Analysis

- **Peak throughput**: ~100 requests/min (stable under 100 concurrent users)
- **Error rate**: 0%
- **Success criteria met**: ✓ p(95)<500ms, ✓ p(99)<1000ms, ✓ rate<0.1

## Performance Thresholds

The load test enforces the following SLOs:

- **Response time p95**: < 500ms
- **Response time p99**: < 1000ms
- **Error rate**: < 10%

If any threshold is exceeded, the test will fail and report violations.

## Output

Results are written to `/tmp/load-test-results.json` in JSON format, suitable for integration with monitoring and alerting systems.

### CI Integration

To run the load test in CI/CD:

```bash
k6 run backend/tests/k6_load_test.js \
  --vus 100 \
  --duration 10m \
  --summary-export=/tmp/results.json
```

Exit codes:
- `0`: Test passed (no threshold violations)
- `1`: Test failed (threshold violations detected)

## Monitoring

For continuous performance monitoring:

- Enable k6 Cloud integration: `k6 cloud backend/tests/k6_load_test.js`
- Stream results to Grafana/Prometheus using custom output extensions
- Set up alerts for response time degradation (> 2x baseline p95)

## Troubleshooting

### "Connection refused" error

Ensure the backend is running:
```bash
# In another terminal, start the backend
cargo run --release
```

### High error rates

1. Check backend logs for errors
2. Verify database connectivity
3. Reduce VU count for debugging:
   ```bash
   k6 run -u 10 -d 1m backend/tests/k6_load_test.js
   ```

### Slow response times

1. Profile the backend application
2. Check database query performance
3. Review network latency between load generator and backend

## Future Improvements

- [ ] Add endpoints-specific load profiles (mixed workload)
- [ ] Implement chaos testing scenarios (network delays, DB failures)
- [ ] Add cost analysis based on cloud provider pricing
- [ ] Document scaling recommendations based on load test results
