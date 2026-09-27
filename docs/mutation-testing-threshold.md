# Mutation Testing Threshold Configuration

## Overview

Mutation testing is a quality assurance technique that measures test coverage by introducing intentional code mutations and checking if tests catch them. The mutation survival rate is the percentage of mutants that survive (i.e., are not killed by tests).

## Threshold Configuration

**Current Threshold**: 75%

This means that at least 75% of code mutations must be caught by tests. If the survival rate falls below this threshold, the CI job will fail, indicating the need for better test coverage.

## Why 75%?

- **Realistic**: Achieves strong test coverage without being unattainable
- **Motivation**: Ensures critical code paths and edge cases are properly tested
- **Feedback**: Highlights areas where test coverage can be improved
- **Flexibility**: The threshold can be adjusted based on project maturity and requirements

## Updating the Threshold

The threshold is defined in `.github/workflows/mutation-testing.yml` in the `THRESHOLD` variable:

```bash
THRESHOLD=75
```

To update the threshold:
1. Modify the value in the workflow file
2. Update this documentation
3. Create a PR and explain the rationale for the change

## How to Check Your Local Mutation Score

Run mutation tests locally before pushing:

```bash
cargo install cargo-mutants
cargo mutants --package ttl-vault
```

This will show the survival rate for your code changes.

## CI Integration

The mutation testing job runs on every pull request and:
1. Executes `cargo mutants --package ttl-vault`
2. Parses the survival rate from the report
3. Compares it against the threshold
4. Fails the job if below threshold (with helpful error message)
5. Uploads the full report as an artifact for analysis

## Failure Response

If the mutation testing threshold is not met:
1. Review the detailed report (available as a CI artifact)
2. Identify which code mutations are surviving (not caught by tests)
3. Improve test coverage for those code paths
4. Run tests locally and re-push

## Related

- Test coverage analysis: See `CONTRIBUTING.md` for testing best practices
- Report location: CI artifacts section → `mutant-survival-report`
