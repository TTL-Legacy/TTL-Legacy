use crate::models::*;
use crate::db::*;
use chrono::Utc;

pub fn get_vault_templates() -> VaultTemplateList {
    VaultTemplateList {
        templates: vec![
            VaultTemplate {
                id: "simple-inheritance".to_string(),
                name: "Simple Inheritance".to_string(),
                description: "Basic vault for single beneficiary inheritance".to_string(),
                check_in_interval: 86400 * 30, // 30 days
                recommended_for: "Individual asset protection".to_string(),
            },
            VaultTemplate {
                id: "family-trust".to_string(),
                name: "Family Trust".to_string(),
                description: "Multi-beneficiary vault for family wealth distribution".to_string(),
                check_in_interval: 86400 * 90, // 90 days
                recommended_for: "Family wealth management".to_string(),
            },
            VaultTemplate {
                id: "business-succession".to_string(),
                name: "Business Succession".to_string(),
                description: "Vault for business continuity and succession planning".to_string(),
                check_in_interval: 86400 * 60, // 60 days
                recommended_for: "Business asset protection".to_string(),
            },
        ],
    }
}

pub fn create_vault_from_template(
    store: &VaultStore,
    template_id: &str,
    owner: String,
    beneficiary: String,
) -> Result<Vault, String> {
    let templates = get_vault_templates();
    let template = templates
        .templates
        .iter()
        .find(|t| t.id == template_id)
        .ok_or_else(|| "Template not found".to_string())?;

    let vault_id = uuid::Uuid::new_v4().to_string();
    let vault = Vault {
        id: vault_id,
        owner,
        beneficiary,
        balance: 0,
        check_in_interval: template.check_in_interval,
        last_check_in: Utc::now(),
        created_at: Utc::now(),
        status: VaultStatus::Active,
        ttl_remaining: Some(template.check_in_interval),
    };

    store.lock().unwrap().insert(vault.id.clone(), vault.clone());
    Ok(vault)
}

// ── Localized email templates ───────────────────────────────────────────────

fn resolve_locale(locale: &Option<Locale>) -> &Locale {
    static EN: Locale = Locale::En;
    locale.as_ref().unwrap_or(&EN)
}

pub fn email_subject(notification_type: &NotificationType, locale: &Option<Locale>) -> &'static str {
    match (resolve_locale(locale), notification_type) {
        // English
        (Locale::En, NotificationType::ExpiryWarning) => "Your vault is expiring soon",
        (Locale::En, NotificationType::CheckInReminder) => "Time to check in to your vault",
        (Locale::En, NotificationType::VaultReleased) => "Your vault has been released",
        (Locale::En, NotificationType::VaultPaused) => "Your vault has been paused",
        // Spanish
        (Locale::Es, NotificationType::ExpiryWarning) => "Tu bóveda está por vencer",
        (Locale::Es, NotificationType::CheckInReminder) => "Es hora de registrarte en tu bóveda",
        (Locale::Es, NotificationType::VaultReleased) => "Tu bóveda ha sido liberada",
        (Locale::Es, NotificationType::VaultPaused) => "Tu bóveda ha sido pausada",
        // French
        (Locale::Fr, NotificationType::ExpiryWarning) => "Votre coffre expire bientôt",
        (Locale::Fr, NotificationType::CheckInReminder) => "Il est temps de vous enregistrer",
        (Locale::Fr, NotificationType::VaultReleased) => "Votre coffre a été libéré",
        (Locale::Fr, NotificationType::VaultPaused) => "Votre coffre a été mis en pause",
        // German
        (Locale::De, NotificationType::ExpiryWarning) => "Ihr Tresor läuft bald ab",
        (Locale::De, NotificationType::CheckInReminder) => "Zeit für Ihren Check-in",
        (Locale::De, NotificationType::VaultReleased) => "Ihr Tresor wurde freigegeben",
        (Locale::De, NotificationType::VaultPaused) => "Ihr Tresor wurde pausiert",
    }
}

pub fn email_body(
    notification_type: &NotificationType,
    locale: &Option<Locale>,
    vault_id: &str,
    hours_remaining: Option<u64>,
) -> String {
    match (resolve_locale(locale), notification_type) {
        // English
        (Locale::En, NotificationType::ExpiryWarning) => {
            let h = hours_remaining.unwrap_or(24);
            format!("Your vault {vault_id} expires in approximately {h} hours. Check in now to keep it active.")
        }
        (Locale::En, NotificationType::CheckInReminder) =>
            format!("Please check in to your vault {vault_id} to keep it active."),
        (Locale::En, NotificationType::VaultReleased) =>
            format!("Vault {vault_id} has been released to the designated beneficiary."),
        (Locale::En, NotificationType::VaultPaused) =>
            format!("Vault {vault_id} has been paused."),
        // Spanish
        (Locale::Es, NotificationType::ExpiryWarning) => {
            let h = hours_remaining.unwrap_or(24);
            format!("Tu bóveda {vault_id} vence en aproximadamente {h} horas. Regístrate ahora para mantenerla activa.")
        }
        (Locale::Es, NotificationType::CheckInReminder) =>
            format!("Por favor regístrate en tu bóveda {vault_id} para mantenerla activa."),
        (Locale::Es, NotificationType::VaultReleased) =>
            format!("La bóveda {vault_id} ha sido liberada al beneficiario designado."),
        (Locale::Es, NotificationType::VaultPaused) =>
            format!("La bóveda {vault_id} ha sido pausada."),
        // French
        (Locale::Fr, NotificationType::ExpiryWarning) => {
            let h = hours_remaining.unwrap_or(24);
            format!("Votre coffre {vault_id} expire dans environ {h} heures. Enregistrez-vous maintenant.")
        }
        (Locale::Fr, NotificationType::CheckInReminder) =>
            format!("Veuillez vous enregistrer dans votre coffre {vault_id} pour le maintenir actif."),
        (Locale::Fr, NotificationType::VaultReleased) =>
            format!("Le coffre {vault_id} a été libéré au bénéficiaire désigné."),
        (Locale::Fr, NotificationType::VaultPaused) =>
            format!("Le coffre {vault_id} a été mis en pause."),
        // German
        (Locale::De, NotificationType::ExpiryWarning) => {
            let h = hours_remaining.unwrap_or(24);
            format!("Ihr Tresor {vault_id} läuft in etwa {h} Stunden ab. Melden Sie sich jetzt an.")
        }
        (Locale::De, NotificationType::CheckInReminder) =>
            format!("Bitte melden Sie sich bei Ihrem Tresor {vault_id} an, um ihn aktiv zu halten."),
        (Locale::De, NotificationType::VaultReleased) =>
            format!("Tresor {vault_id} wurde an den designierten Begünstigten freigegeben."),
        (Locale::De, NotificationType::VaultPaused) =>
            format!("Tresor {vault_id} wurde pausiert."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_vault_templates() {
        let templates = get_vault_templates();
        assert_eq!(templates.templates.len(), 3);
        assert!(templates.templates.iter().any(|t| t.id == "simple-inheritance"));
        assert!(templates.templates.iter().any(|t| t.id == "family-trust"));
        assert!(templates.templates.iter().any(|t| t.id == "business-succession"));
    }

    #[test]
    fn test_simple_inheritance_template() {
        let templates = get_vault_templates();
        let template = templates
            .templates
            .iter()
            .find(|t| t.id == "simple-inheritance")
            .unwrap();
        assert_eq!(template.check_in_interval, 86400 * 30);
        assert!(template.description.contains("single beneficiary"));
    }

    #[test]
    fn test_family_trust_template() {
        let templates = get_vault_templates();
        let template = templates
            .templates
            .iter()
            .find(|t| t.id == "family-trust")
            .unwrap();
        assert_eq!(template.check_in_interval, 86400 * 90);
        assert!(template.description.contains("Multi-beneficiary"));
    }

    #[test]
    fn test_business_succession_template() {
        let templates = get_vault_templates();
        let template = templates
            .templates
            .iter()
            .find(|t| t.id == "business-succession")
            .unwrap();
        assert_eq!(template.check_in_interval, 86400 * 60);
        assert!(template.description.contains("business"));
    }

    #[test]
    fn test_create_vault_from_template() {
        let store = create_vault_store();
        let result = create_vault_from_template(
            &store,
            "simple-inheritance",
            "owner1".to_string(),
            "ben1".to_string(),
        );
        assert!(result.is_ok());
        let vault = result.unwrap();
        assert_eq!(vault.owner, "owner1");
        assert_eq!(vault.beneficiary, "ben1");
        assert_eq!(vault.check_in_interval, 86400 * 30);
    }

    #[test]
    fn test_create_vault_from_family_trust_template() {
        let store = create_vault_store();
        let result = create_vault_from_template(
            &store,
            "family-trust",
            "owner1".to_string(),
            "ben1".to_string(),
        );
        assert!(result.is_ok());
        let vault = result.unwrap();
        assert_eq!(vault.check_in_interval, 86400 * 90);
    }

    #[test]
    fn test_create_vault_from_invalid_template() {
        let store = create_vault_store();
        let result = create_vault_from_template(
            &store,
            "invalid-template",
            "owner1".to_string(),
            "ben1".to_string(),
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    // ── Locale-aware email template tests ───────────────────────────────────

    #[test]
    fn test_english_expiry_subject() {
        let subject = email_subject(&NotificationType::ExpiryWarning, &None);
        assert!(subject.contains("expiring"));
    }

    #[test]
    fn test_english_is_default_locale() {
        let subject_none = email_subject(&NotificationType::ExpiryWarning, &None);
        let subject_en = email_subject(&NotificationType::ExpiryWarning, &Some(Locale::En));
        assert_eq!(subject_none, subject_en);
    }

    #[test]
    fn test_spanish_templates() {
        let locale = Some(Locale::Es);
        let subject = email_subject(&NotificationType::ExpiryWarning, &locale);
        assert!(subject.contains("vencer"));
        let body = email_body(&NotificationType::ExpiryWarning, &locale, "v1", Some(12));
        assert!(body.contains("12"));
        assert!(body.contains("v1"));
    }

    #[test]
    fn test_french_templates() {
        let locale = Some(Locale::Fr);
        let subject = email_subject(&NotificationType::CheckInReminder, &locale);
        assert!(subject.contains("enregistrer"));
        let body = email_body(&NotificationType::CheckInReminder, &locale, "v1", None);
        assert!(body.contains("v1"));
    }

    #[test]
    fn test_german_templates() {
        let locale = Some(Locale::De);
        let subject = email_subject(&NotificationType::VaultReleased, &locale);
        assert!(subject.contains("freigegeben"));
        let body = email_body(&NotificationType::VaultReleased, &locale, "v1", None);
        assert!(body.contains("v1"));
    }

    #[test]
    fn test_all_notification_types_have_templates() {
        let types = [
            NotificationType::ExpiryWarning,
            NotificationType::CheckInReminder,
            NotificationType::VaultReleased,
            NotificationType::VaultPaused,
        ];
        let locales = [
            Some(Locale::En),
            Some(Locale::Es),
            Some(Locale::Fr),
            Some(Locale::De),
        ];
        for locale in &locales {
            for nt in &types {
                let subject = email_subject(nt, locale);
                assert!(!subject.is_empty());
                let body = email_body(nt, locale, "v1", Some(24));
                assert!(!body.is_empty());
                assert!(body.contains("v1"));
            }
        }
    }

    #[test]
    fn test_fallback_to_english() {
        let subject = email_subject(&NotificationType::VaultPaused, &None);
        let subject_en = email_subject(&NotificationType::VaultPaused, &Some(Locale::En));
        assert_eq!(subject, subject_en);
    }

    #[test]
    fn test_expiry_body_includes_hours() {
        let body = email_body(&NotificationType::ExpiryWarning, &Some(Locale::En), "v1", Some(6));
        assert!(body.contains("6"));
    }

    #[test]
    fn test_expiry_body_defaults_to_24_hours() {
        let body = email_body(&NotificationType::ExpiryWarning, &Some(Locale::En), "v1", None);
        assert!(body.contains("24"));
    }
}
