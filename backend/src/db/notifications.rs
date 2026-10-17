use super::*;

// ── Task 4: Notification Preferences ─────────────────────────────────────────

pub fn set_notification_preferences(
    notif_store: &NotificationStore,
    prefs: crate::models::VaultNotificationPreferences,
) {
    notif_store
        .lock()
        .unwrap()
        .insert(prefs.owner.clone(), prefs);
}

pub fn get_notification_preferences(
    notif_store: &NotificationStore,
    owner: &str,
) -> Option<crate::models::VaultNotificationPreferences> {
    notif_store.lock().unwrap().get(owner).cloned()
}

impl Db {
    pub fn upsert(&self, prefs: &ReminderPreferences) -> Result<(), rusqlite::Error> {
        let channels_json = serde_json::to_string(&prefs.channels).unwrap();
        self.conn.lock().unwrap().execute(
            r#"
            INSERT INTO reminder_preferences (vault_id, channels, hours_before_expiry, frequency)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(vault_id) DO UPDATE SET
              channels = excluded.channels,
              hours_before_expiry = excluded.hours_before_expiry,
              frequency = excluded.frequency,
              deleted_at = NULL
            "#,
            params![
                prefs.vault_id as i64,
                channels_json,
                prefs.hours_before_expiry as i64,
                serde_json::to_string(&prefs.frequency).unwrap(),
            ],
        )?;
        Ok(())
    }

    pub fn get(&self, vault_id: u64) -> Result<ReminderPreferences, rusqlite::Error> {
        let binding = self.conn.lock().unwrap();
        let mut stmt = binding.prepare(
            r#"SELECT vault_id, channels, hours_before_expiry, frequency, deleted_at
               FROM reminder_preferences
               WHERE vault_id = ?1 AND deleted_at IS NULL"#,
        )?;
        let row = stmt.query_row(params![vault_id as i64], |r| {
            let channels_str: String = r.get(1)?;
            let frequency_str: String = r.get(3)?;
            let channels: Vec<Channel> = serde_json::from_str(&channels_str).unwrap_or_default();
            let frequency: Frequency = serde_json::from_str(&frequency_str).unwrap();
            Ok(ReminderPreferences {
                vault_id: r.get::<_, i64>(0)? as u64,
                channels,
                hours_before_expiry: r.get::<_, i64>(2)? as u32,
                frequency,
                deleted_at: None,
            })
        })?;
        Ok(row)
    }

    pub fn all(&self) -> Result<Vec<ReminderPreferences>, rusqlite::Error> {
        let binding = self.conn.lock().unwrap();
        let mut stmt = binding.prepare(
            r#"SELECT vault_id, channels, hours_before_expiry, frequency, deleted_at
               FROM reminder_preferences
               WHERE deleted_at IS NULL"#,
        )?;
        let iter = stmt.query_map([], |r| {
            let channels_str: String = r.get(1)?;
            let frequency_str: String = r.get(3)?;
            let channels: Vec<Channel> = serde_json::from_str(&channels_str).unwrap_or_default();
            let frequency: Frequency = serde_json::from_str(&frequency_str).unwrap();
            Ok(ReminderPreferences {
                vault_id: r.get::<_, i64>(0)? as u64,
                channels,
                hours_before_expiry: r.get::<_, i64>(2)? as u32,
                frequency,
                deleted_at: None,
            })
        })?;

        let mut out = Vec::new();
        for item in iter {
            out.push(item?);
        }
        Ok(out)
    }

    pub fn soft_delete_reminder(&self, vault_id: u64) -> Result<(), rusqlite::Error> {
        self.conn.lock().unwrap().execute(
            "UPDATE reminder_preferences SET deleted_at = ?1 WHERE vault_id = ?2 AND deleted_at IS NULL",
            params![chrono::Utc::now().to_rfc3339(), vault_id as i64],
        )?;
        Ok(())
    }

    pub fn all_reminders_including_deleted(
        &self,
        vault_id: u64,
    ) -> Result<Vec<ReminderPreferences>, rusqlite::Error> {
        let binding = self.conn.lock().unwrap();
        let mut stmt = binding.prepare(
            r#"SELECT vault_id, channels, hours_before_expiry, frequency, deleted_at
               FROM reminder_preferences
               WHERE vault_id = ?1"#,
        )?;
        let iter = stmt.query_map(params![vault_id as i64], |r| {
            let channels_str: String = r.get(1)?;
            let frequency_str: String = r.get(3)?;
            let channels: Vec<Channel> = serde_json::from_str(&channels_str).unwrap_or_default();
            let frequency: Frequency = serde_json::from_str(&frequency_str).unwrap();
            let deleted_at_str: Option<String> = r.get(4)?;
            let deleted_at = deleted_at_str.and_then(|s| {
                chrono::DateTime::parse_from_rfc3339(&s)
                    .ok()
                    .map(|dt| dt.with_timezone(&chrono::Utc))
            });
            Ok(ReminderPreferences {
                vault_id: r.get::<_, i64>(0)? as u64,
                channels,
                hours_before_expiry: r.get::<_, i64>(2)? as u32,
                frequency,
                deleted_at,
            })
        })?;

        let mut out = Vec::new();
        for item in iter {
            out.push(item?);
        }
        Ok(out)
    }

    pub fn upsert_subscription(&self, sub: &Subscription) -> Result<(), rusqlite::Error> {
        let channels_json = serde_json::to_string(&sub.channels).unwrap();
        let frequency_json = serde_json::to_string(&sub.frequency).unwrap();
        self.conn.lock().unwrap().execute(
            r#"
            INSERT INTO vault_subscriptions (vault_id, owner, channels, frequency)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(vault_id) DO UPDATE SET
              owner = excluded.owner,
              channels = excluded.channels,
              frequency = excluded.frequency
            "#,
            params![
                sub.vault_id as i64,
                sub.owner,
                channels_json,
                frequency_json,
            ],
        )?;
        Ok(())
    }

    pub fn delete_subscription(&self, vault_id: u64) -> Result<(), rusqlite::Error> {
        self.conn.lock().unwrap().execute(
            "DELETE FROM vault_subscriptions WHERE vault_id = ?1",
            params![vault_id as i64],
        )?;
        Ok(())
    }

    // ── Withdrawal alert preferences ─────────────────────────────────────────

    /// Upsert per-vault withdrawal alert notification preferences.
    pub fn upsert_withdrawal_alert_prefs(
        &self,
        prefs: &crate::models::WithdrawalAlertPreferences,
    ) -> Result<(), rusqlite::Error> {
        self.conn.lock().unwrap().execute(
            r#"
            INSERT INTO withdrawal_alert_preferences (vault_id, owner, email_enabled, push_enabled)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(vault_id) DO UPDATE SET
                owner         = excluded.owner,
                email_enabled = excluded.email_enabled,
                push_enabled  = excluded.push_enabled
            "#,
            params![
                prefs.vault_id as i64,
                prefs.owner,
                prefs.email_enabled as i64,
                prefs.push_enabled as i64,
            ],
        )?;
        Ok(())
    }

    /// Retrieve withdrawal alert preferences for a vault.
    /// Returns `None` if no record exists (owner has never configured prefs).
    pub fn get_withdrawal_alert_prefs(
        &self,
        vault_id: u64,
    ) -> Result<Option<crate::models::WithdrawalAlertPreferences>, rusqlite::Error> {
        let binding = self.conn.lock().unwrap();
        let mut stmt = binding.prepare(
            r#"SELECT vault_id, owner, email_enabled, push_enabled
               FROM withdrawal_alert_preferences
               WHERE vault_id = ?1"#,
        )?;
        let row = stmt.query_row(params![vault_id as i64], |r| {
            Ok(crate::models::WithdrawalAlertPreferences {
                vault_id: r.get::<_, i64>(0)? as u64,
                owner: r.get(1)?,
                email_enabled: r.get::<_, i64>(2)? != 0,
                push_enabled: r.get::<_, i64>(3)? != 0,
            })
        });
        match row {
            Ok(p) => Ok(Some(p)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Delete withdrawal alert preferences for a vault.
    pub fn delete_withdrawal_alert_prefs(&self, vault_id: u64) -> Result<(), rusqlite::Error> {
        self.conn.lock().unwrap().execute(
            "DELETE FROM withdrawal_alert_preferences WHERE vault_id = ?1",
            params![vault_id as i64],
        )?;
        Ok(())
    }

    pub fn get_subscription(&self, vault_id: u64) -> Result<Option<Subscription>, rusqlite::Error> {
        let binding = self.conn.lock().unwrap();
        let mut stmt = binding.prepare(
            r#"SELECT vault_id, owner, channels, frequency
               FROM vault_subscriptions
               WHERE vault_id = ?1"#,
        )?;
        let row = stmt.query_row(params![vault_id as i64], |r| {
            let channels_str: String = r.get(2)?;
            let frequency_str: String = r.get(3)?;
            let channels: Vec<SubscriptionChannel> =
                serde_json::from_str(&channels_str).unwrap_or_default();
            let frequency: SubscriptionFrequency = serde_json::from_str(&frequency_str).unwrap();
            Ok(Subscription {
                vault_id: r.get::<_, i64>(0)? as u64,
                owner: r.get(1)?,
                channels,
                frequency,
            })
        });
        match row {
            Ok(sub) => Ok(Some(sub)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    // ── Unsubscribe (#828) ──────────────────────────────────────────────────

    pub fn store_unsubscribe_token(&self, token: &str, owner: &str) {
        let _ = self.conn.lock().unwrap().execute(
            r#"INSERT OR REPLACE INTO unsubscribe_tokens (token, owner, created_at)
               VALUES (?1, ?2, ?3)"#,
            params![token, owner, chrono::Utc::now().to_rfc3339()],
        );
    }

    pub fn process_unsubscribe(&self, token: &str) -> Result<String, String> {
        let conn = self.conn.lock().unwrap();
        let owner: String = conn
            .query_row(
                "SELECT owner FROM unsubscribe_tokens WHERE token = ?1",
                params![token],
                |r| r.get(0),
            )
            .map_err(|_| "invalid or expired unsubscribe token".to_string())?;

        conn.execute(
            "INSERT OR IGNORE INTO unsubscribed_users (owner) VALUES (?1)",
            params![&owner],
        )
        .map_err(|e| e.to_string())?;

        Ok(owner)
    }

    pub fn is_unsubscribed(&self, owner: &str) -> bool {
        self.conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT 1 FROM unsubscribed_users WHERE owner = ?1",
                params![owner],
                |_| Ok(()),
            )
            .is_ok()
    }

    pub fn generate_unsubscribe_token(&self, owner: &str) -> String {
        let token = uuid::Uuid::new_v4().to_string();
        self.store_unsubscribe_token(&token, owner);
        token
    }

    // ── Token-based reminder links (#1286) ──────────────────────────────────

    pub fn store_reminder_token(&self, token: &str, vault_id: &str, owner: &str) {
        let _ = self.conn.lock().unwrap().execute(
            r#"INSERT OR REPLACE INTO reminder_tokens (token, vault_id, owner, created_at)
               VALUES (?1, ?2, ?3, ?4)"#,
            params![token, vault_id, owner, chrono::Utc::now().to_rfc3339()],
        );
    }

    pub fn generate_reminder_token(&self, vault_id: &str, owner: &str) -> String {
        let token = uuid::Uuid::new_v4().to_string();
        self.store_reminder_token(&token, vault_id, owner);
        token
    }

    pub fn resolve_reminder_token(&self, token: &str) -> Result<(String, String), String> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT vault_id, owner FROM reminder_tokens WHERE token = ?1",
            params![token],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|_| "invalid or expired reminder token".to_string())
    }

    pub fn generate_reminder_url(&self, vault_id: &str, owner: &str, base_url: &str) -> String {
        let token = self.generate_reminder_token(vault_id, owner);
        format!("{base_url}/reminders/check-in?token={token}")
    }

    // ── Issue #1337: Beneficiary archival notification ─────────────────────

    /// Persist (create or update) a beneficiary's contact information.
    /// The `vault_id` + `beneficiary_address` pair is the primary key.
    pub fn upsert_beneficiary_contact(
        &self,
        contact: &crate::models::BeneficiaryContact,
    ) -> Result<(), rusqlite::Error> {
        self.conn.lock().unwrap().execute(
            r#"
            INSERT INTO beneficiary_contacts
                (vault_id, beneficiary_address, email, phone, opted_in, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(vault_id, beneficiary_address) DO UPDATE SET
                email      = excluded.email,
                phone      = excluded.phone,
                opted_in   = excluded.opted_in,
                updated_at = excluded.updated_at
            "#,
            params![
                contact.vault_id,
                contact.beneficiary_address,
                contact.email,
                contact.phone,
                contact.opted_in as i64,
                contact.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    /// Retrieve the contact entry for a specific vault + beneficiary pair.
    pub fn get_beneficiary_contact(
        &self,
        vault_id: &str,
        beneficiary_address: &str,
    ) -> Result<Option<crate::models::BeneficiaryContact>, rusqlite::Error> {
        let binding = self.conn.lock().unwrap();
        let mut stmt = binding.prepare(
            r#"
            SELECT vault_id, beneficiary_address, email, phone, opted_in, updated_at
            FROM beneficiary_contacts
            WHERE vault_id = ?1 AND beneficiary_address = ?2
            "#,
        )?;

        let row_res = stmt.query_row(params![vault_id, beneficiary_address], |r| {
            let updated_at_str: String = r.get(5)?;
            let updated_at = chrono::DateTime::parse_from_rfc3339(&updated_at_str)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        5,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;
            let opted_in_i: i64 = r.get(4)?;
            Ok(crate::models::BeneficiaryContact {
                vault_id: r.get(0)?,
                beneficiary_address: r.get(1)?,
                email: r.get(2)?,
                phone: r.get(3)?,
                opted_in: opted_in_i != 0,
                updated_at,
            })
        });

        match row_res {
            Ok(c) => Ok(Some(c)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Retrieve all opted-in beneficiary contacts for a given vault.
    pub fn get_opted_in_contacts_for_vault(
        &self,
        vault_id: &str,
    ) -> Result<Vec<crate::models::BeneficiaryContact>, rusqlite::Error> {
        let binding = self.conn.lock().unwrap();
        let mut stmt = binding.prepare(
            r#"
            SELECT vault_id, beneficiary_address, email, phone, opted_in, updated_at
            FROM beneficiary_contacts
            WHERE vault_id = ?1 AND opted_in = 1
            "#,
        )?;

        let iter = stmt.query_map(params![vault_id], |r| {
            let updated_at_str: String = r.get(5)?;
            let updated_at = chrono::DateTime::parse_from_rfc3339(&updated_at_str)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        5,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;
            let opted_in_i: i64 = r.get(4)?;
            Ok(crate::models::BeneficiaryContact {
                vault_id: r.get(0)?,
                beneficiary_address: r.get(1)?,
                email: r.get(2)?,
                phone: r.get(3)?,
                opted_in: opted_in_i != 0,
                updated_at,
            })
        })?;

        let mut out = Vec::new();
        for item in iter {
            out.push(item?);
        }
        Ok(out)
    }

    /// Update only the `opted_in` flag for a beneficiary contact.
    pub fn set_beneficiary_notification_opt_in(
        &self,
        vault_id: &str,
        beneficiary_address: &str,
        opted_in: bool,
    ) -> Result<(), rusqlite::Error> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.lock().unwrap().execute(
            r#"
            UPDATE beneficiary_contacts
            SET opted_in = ?1, updated_at = ?2
            WHERE vault_id = ?3 AND beneficiary_address = ?4
            "#,
            params![opted_in as i64, now, vault_id, beneficiary_address],
        )?;
        Ok(())
    }

    /// Record a dispatched archival notification for audit purposes.
    pub fn record_beneficiary_archival_notification(
        &self,
        notif: &crate::models::BeneficiaryArchivalNotification,
    ) -> Result<(), rusqlite::Error> {
        let status = format!("{:?}", notif.status).to_lowercase();
        self.conn.lock().unwrap().execute(
            r#"
            INSERT OR REPLACE INTO beneficiary_archival_notifications
                (id, vault_id, beneficiary_address, channel, dispatched_at, status, error)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                notif.id,
                notif.vault_id,
                notif.beneficiary_address,
                notif.channel,
                notif.dispatched_at.to_rfc3339(),
                status,
                notif.error,
            ],
        )?;
        Ok(())
    }
}
