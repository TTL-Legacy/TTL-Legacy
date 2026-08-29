module.exports = {
  // Extend the conventional commits ruleset
  extends: ["@commitlint/config-conventional"],
  rules: {
    // Allowed commit types for TTL-Legacy
    "type-enum": [
      2,
      "always",
      [
        "feat",     // New feature
        "fix",      // Bug fix
        "docs",     // Documentation changes
        "style",    // Formatting, missing semicolons, etc.
        "refactor", // Code refactor without feature or fix
        "perf",     // Performance improvement
        "test",     // Adding or updating tests
        "build",    // Build system changes
        "ci",       // CI/CD configuration changes
        "chore",    // Other changes that do not modify src or test files
        "revert",   // Reverts a previous commit
        "security", // Security fix or hardening
        "sec",      // Alias for security
      ],
    ],
    // Subject line must not end with a period
    "subject-full-stop": [2, "never", "."],
    // Subject line must be lowercase
    "subject-case": [2, "never", ["sentence-case", "start-case", "pascal-case", "upper-case"]],
    // Header must not exceed 100 characters
    "header-max-length": [2, "always", 100],
  },
};
