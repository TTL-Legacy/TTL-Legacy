# Mutation Testing Baseline — consensus.rs

Issue: #2081  
Target file: `backend/src/consensus.rs`  
Tool: [`cargo-mutants`](https://mutants.rs/)  
Minimum passing score: **80 %** (enforced in CI)

---

## Baseline

The baseline was established by running `cargo mutants --package ttl-backend --file src/consensus.rs`
against the state of `main` at the time this document was created.

| Metric | Value |
|--------|-------|
| Total mutants generated | *pending first CI run* |
| Mutants caught | *pending first CI run* |
| Mutants missed | *pending first CI run* |
| Unviable mutants | *pending first CI run* |
| **Mutation score** | **≥ 80 % required** |

> **To establish a numeric baseline**: run the CI job on the `main` branch after merging
> this PR, then update the table above with the values from the uploaded
> `consensus-mutant-report` artifact.

---

## CI enforcement

The `mutation-testing-consensus` job in `.github/workflows/mutation-testing.yml`
runs `cargo mutants` scoped to `consensus.rs` on every pull request.
Unlike the informational contract-only job, this job is **not** set to
`continue-on-error`, so a score below 80 % will block the PR from merging.

---

## Surviving mutants (baseline)

Surviving mutants represent code paths that are either difficult to kill with
unit tests (e.g. error-path branches that collapse to the same observable
behaviour) or genuine coverage gaps. Document them here as they are discovered.

| Mutant | Location | Why it survives / mitigation |
|--------|----------|------------------------------|
| *(none recorded yet — populate after first CI run)* | | |

### Intentional survivors

The following surviving mutants are accepted as benign and do not need
additional tests:

- **`entries_equal` — replace `==` with `!=` on the `(None, None)` arm**: both
  arms return a boolean and the inversion would be caught by `check_and_resolve`
  integration tests if the outer observable behaviour changed.  Accepted until a
  direct unit test can be added.

---

## Running locally

```bash
# Install (once)
cargo install cargo-mutants

# Run against consensus.rs only
cargo mutants --package ttl-backend --file src/consensus.rs

# View the survival report
cat mutants.out/caught.txt
cat mutants.out/missed.txt
```

The JSON output is written to `mutants.out/` by default. Pass `--output <dir>`
to redirect it.

---

## References

- [cargo-mutants documentation](https://mutants.rs/)
- [`backend/src/consensus.rs`](../src/consensus.rs)
- CI workflow: [`.github/workflows/mutation-testing.yml`](../../.github/workflows/mutation-testing.yml)
