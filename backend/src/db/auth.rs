use super::*;

impl Db {
    // ── Refresh token rotation (Issue #1177) ─────────────────────────────────

    pub fn insert_refresh_token(
        &self,
        jti: &str,
        family_id: &str,
        sub: &str,
        expires_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), rusqlite::Error> {
        self.conn.lock().unwrap().execute(
            r#"INSERT INTO refresh_tokens (jti, family_id, sub, created_at, expires_at, revoked_at)
               VALUES (?1, ?2, ?3, ?4, ?5, NULL)"#,
            params![
                jti,
                family_id,
                sub,
                chrono::Utc::now().to_rfc3339(),
                expires_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    /// Returns (family_id, sub, revoked) for a refresh token, or None if it
    /// was never issued (unknown / already garbage-collected).
    pub fn get_refresh_token(
        &self,
        jti: &str,
    ) -> Result<Option<(String, String, bool)>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT family_id, sub, revoked_at FROM refresh_tokens WHERE jti = ?1")?;
        let row = stmt.query_row(params![jti], |r| {
            let revoked_at: Option<String> = r.get(2)?;
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                revoked_at.is_some(),
            ))
        });
        match row {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn revoke_refresh_token(&self, jti: &str) -> Result<(), rusqlite::Error> {
        self.conn.lock().unwrap().execute(
            "UPDATE refresh_tokens SET revoked_at = ?1 WHERE jti = ?2 AND revoked_at IS NULL",
            params![chrono::Utc::now().to_rfc3339(), jti],
        )?;
        Ok(())
    }

    /// Revokes every token in a rotation family — used when a refresh token
    /// that was already rotated out gets presented again, which indicates
    /// the token may have been stolen and replayed.
    pub fn revoke_refresh_token_family(&self, family_id: &str) -> Result<(), rusqlite::Error> {
        self.conn.lock().unwrap().execute(
            "UPDATE refresh_tokens SET revoked_at = ?1 WHERE family_id = ?2 AND revoked_at IS NULL",
            params![chrono::Utc::now().to_rfc3339(), family_id],
        )?;
        Ok(())
    }
}
