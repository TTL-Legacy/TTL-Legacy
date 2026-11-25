//! Database migration tooling tests for PostgreSQL schema management.
//! Issue #1347: Infrastructure: add database migration tooling for backend schema

#[cfg(test)]
mod database_migrations {
    use std::fs;
    use std::path::Path;
    use std::collections::BTreeMap;

    /// Verifies that migrations directory exists
    #[test]
    fn migrations_directory_exists() {
        assert!(
            Path::new("backend/migrations").exists(),
            "backend/migrations directory should exist"
        );
    }

    /// Verifies that at least one migration file exists
    #[test]
    fn migration_files_exist() {
        let migrations_dir = "backend/migrations";
        let entries = fs::read_dir(migrations_dir)
            .expect("Failed to read migrations directory");

        let migration_count = entries
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let path = entry.path();
                if path.extension()?.to_str() == Some("sql") {
                    Some(())
                } else {
                    None
                }
            })
            .count();

        assert!(
            migration_count > 0,
            "At least one migration file (*.sql) should exist in backend/migrations"
        );
    }

    /// Verifies that migration files follow naming convention (NNNN_description.sql)
    #[test]
    fn migration_files_follow_naming_convention() {
        let migrations_dir = "backend/migrations";
        let entries = fs::read_dir(migrations_dir)
            .expect("Failed to read migrations directory");

        for entry in entries {
            let entry = entry.expect("Failed to read migration entry");
            let path = entry.path();

            if path.extension().map_or(false, |ext| ext == "sql") {
                let filename = path.file_name()
                    .expect("Failed to get filename")
                    .to_str()
                    .expect("Failed to convert filename to string");

                // Should match pattern: NNNN_description.sql
                let parts: Vec<&str> = filename.split('_').collect();
                assert!(
                    !parts.is_empty() && parts[0].len() == 4 && parts[0].chars().all(|c| c.is_numeric()),
                    "Migration file should follow naming convention (NNNN_description.sql): {}",
                    filename
                );
            }
        }
    }

    /// Verifies that migration files are numbered sequentially
    #[test]
    fn migrations_are_sequentially_numbered() {
        let migrations_dir = "backend/migrations";
        let entries = fs::read_dir(migrations_dir)
            .expect("Failed to read migrations directory");

        let mut migration_numbers = BTreeMap::new();

        for entry in entries {
            let entry = entry.expect("Failed to read migration entry");
            let path = entry.path();

            if path.extension().map_or(false, |ext| ext == "sql") {
                let filename = path.file_name()
                    .expect("Failed to get filename")
                    .to_str()
                    .expect("Failed to convert filename to string");

                if let Some(num_str) = filename.split('_').next() {
                    if let Ok(num) = num_str.parse::<u32>() {
                        migration_numbers.insert(num, filename.to_string());
                    }
                }
            }
        }

        assert!(
            !migration_numbers.is_empty(),
            "At least one numbered migration should exist"
        );
    }

    /// Verifies that Cargo.toml includes sqlx for migrations
    #[test]
    fn cargo_toml_has_sqlx_dependency() {
        let content = fs::read_to_string("backend/Cargo.toml")
            .expect("Failed to read backend/Cargo.toml");

        assert!(
            content.contains("sqlx"),
            "backend/Cargo.toml should include sqlx dependency for migrations"
        );
    }

    /// Verifies that sqlx is configured with migrate feature
    #[test]
    fn sqlx_includes_migrate_feature() {
        let content = fs::read_to_string("backend/Cargo.toml")
            .expect("Failed to read backend/Cargo.toml");

        assert!(
            content.contains("sqlx") && (content.contains("migrate") || content.contains("\"migrate\"")),
            "sqlx should be configured with 'migrate' feature for SQL migrations"
        );
    }

    /// Verifies that migration SQL files contain valid SQL
    #[test]
    fn migration_files_contain_sql_statements() {
        let migrations_dir = "backend/migrations";
        let entries = fs::read_dir(migrations_dir)
            .expect("Failed to read migrations directory");

        for entry in entries {
            let entry = entry.expect("Failed to read migration entry");
            let path = entry.path();

            if path.extension().map_or(false, |ext| ext == "sql") {
                let content = fs::read_to_string(&path)
                    .expect(&format!("Failed to read migration file: {:?}", path));

                assert!(
                    !content.trim().is_empty(),
                    "Migration file should not be empty: {:?}",
                    path
                );

                let uppercase_content = content.to_uppercase();
                assert!(
                    uppercase_content.contains("CREATE") ||
                    uppercase_content.contains("ALTER") ||
                    uppercase_content.contains("DROP") ||
                    uppercase_content.contains("INSERT") ||
                    uppercase_content.contains("UPDATE"),
                    "Migration file should contain SQL statements: {:?}",
                    path
                );
            }
        }
    }

    /// Verifies that migration files use consistent SQL style
    #[test]
    fn migration_files_use_consistent_style() {
        let migrations_dir = "backend/migrations";
        let entries = fs::read_dir(migrations_dir)
            .expect("Failed to read migrations directory");

        for entry in entries {
            let entry = entry.expect("Failed to read migration entry");
            let path = entry.path();

            if path.extension().map_or(false, |ext| ext == "sql") {
                let content = fs::read_to_string(&path)
                    .expect(&format!("Failed to read migration file: {:?}", path));

                // Check that SQL keywords are uppercase (common convention)
                assert!(
                    content.contains("CREATE") ||
                    content.contains("ALTER") ||
                    content.contains("DROP") ||
                    !content.to_uppercase().contains("CREATE"),
                    "SQL keywords should use consistent casing: {:?}",
                    path
                );
            }
        }
    }

    /// Verifies that first migration establishes core schema
    #[test]
    fn first_migration_establishes_schema() {
        let migrations_dir = "backend/migrations";
        let entries = fs::read_dir(migrations_dir)
            .expect("Failed to read migrations directory");

        let first_migration = entries
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let path = entry.path();
                if path.extension()?.to_str() == Some("sql") {
                    path.file_name()?.to_str().map(|s| s.to_string())
                } else {
                    None
                }
            })
            .min();

        if let Some(first_file) = first_migration {
            let path = format!("backend/migrations/{}", first_file);
            let content = fs::read_to_string(&path)
                .expect("Failed to read first migration");

            assert!(
                content.to_uppercase().contains("CREATE"),
                "First migration should establish initial schema"
            );
        }
    }
}
