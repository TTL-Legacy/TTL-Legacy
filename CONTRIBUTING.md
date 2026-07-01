# Contributing to TTL-Legacy

We welcome contributions! Please follow these guidelines to help us maintain project quality.

## Development Workflow
1. **Fork the repo** and create your branch: git checkout -b feature/your-feature-name.
2. **Formatting:** We use ustfmt. Please run the following command before committing:
   \\\ash
   cargo fmt
   \\\
3. **Testing:** Run the test suite before submitting:
   \\\ash
   ./scripts/test.sh
   \\\
4. **Pull Requests:** Open a PR against main. Ensure your PR description clearly outlines the changes and links to the relevant issue.

## Style Guide
- Follow standard Rust idiomatic practices.
- Use /// for all public function documentation.
- Maintain consistency with the existing project structure.
