use crate::notifications::NotificationType;

/// Resolve the locale for a notification, falling back to English when the
/// requested locale is not supported.
pub fn resolve_locale(locale: Option<&str>) -> &'static str {
    match locale {
        Some("en") => "en",
        Some("es") => "es",
        Some("fr") => "fr",
        Some("de") => "de",
        _ => "en",
    }
}

/// Build the subject line for an email notification.
pub fn email_subject(notification_type: &NotificationType, locale: Option<&str>) -> String {
    let locale = resolve_locale(locale);
    match (notification_type, locale) {
        (NotificationType::Welcome, "es") => "Bienvenido".to_string(),
        (NotificationType::Welcome, "fr") => "Bienvenue".to_string(),
        (NotificationType::Welcome, "de") => "Willkommen".to_string(),
        (NotificationType::Welcome, _) => "Welcome".to_string(),
        (NotificationType::PasswordReset, "es") => "Restablecer contraseña".to_string(),
        (NotificationType::PasswordReset, "fr") => "Réinitialiser le mot de passe".to_string(),
        (NotificationType::PasswordReset, "de") => "Passwort zurücksetzen".to_string(),
        (NotificationType::PasswordReset, _) => "Reset your password".to_string(),
        (NotificationType::AccountUpdate, "es") => "Actualización de cuenta".to_string(),
        (NotificationType::AccountUpdate, "fr") => "Mise à jour du compte".to_string(),
        (NotificationType::AccountUpdate, "de") => "Kontoaktualisierung".to_string(),
        (NotificationType::AccountUpdate, _) => "Account update".to_string(),
    }
}

/// Build the body for an email notification.
pub fn email_body(
    notification_type: &NotificationType,
    locale: Option<&str>,
    name: &str,
) -> String {
    let locale = resolve_locale(locale);
    match (notification_type, locale) {
        (NotificationType::Welcome, "es") => {
            format!("Hola {}, ¡gracias por registrarte!", name)
        }
        (NotificationType::Welcome, "fr") => {
            format!("Bonjour {}, merci de votre inscription !", name)
        }
        (NotificationType::Welcome, "de") => {
            format!("Hallo {}, danke für deine Registrierung!", name)
        }
        (NotificationType::Welcome, _) => {
            format!("Hello {}, thanks for signing up!", name)
        }
        (NotificationType::PasswordReset, "es") => {
            format!("Hola {}, haz clic en el enlace para restablecer tu contraseña.", name)
        }
        (NotificationType::PasswordReset, "fr") => {
            format!("Bonjour {}, cliquez sur le lien pour réinitialiser votre mot de passe.", name)
        }
        (NotificationType::PasswordReset, "de") => {
            format!("Hallo {}, klicke auf den Link, um dein Passwort zurückzusetzen.", name)
        }
        (NotificationType::PasswordReset, _) => {
            format!("Hello {}, click the link to reset your password.", name)
        }
        (NotificationType::AccountUpdate, "es") => {
            format!("Hola {}, tu cuenta ha sido actualizada.", name)
        }
        (NotificationType::AccountUpdate, "fr") => {
            format!("Bonjour {}, votre compte a été mis à jour.", name)
        }
        (NotificationType::AccountUpdate, "de") => {
            format!("Hallo {}, dein Konto wurde aktualisiert.", name)
        }
        (NotificationType::AccountUpdate, _) => {
            format!("Hello {}, your account has been updated.", name)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_locale_defaults_to_english() {
        assert_eq!(resolve_locale(None), "en");
        assert_eq!(resolve_locale(Some("xx")), "en");
    }

    #[test]
    fn resolve_locale_supports_known_locales() {
        assert_eq!(resolve_locale(Some("en")), "en");
        assert_eq!(resolve_locale(Some("es")), "es");
        assert_eq!(resolve_locale(Some("fr")), "fr");
        assert_eq!(resolve_locale(Some("de")), "de");
    }

    #[test]
    fn email_subject_is_localized() {
        assert_eq!(email_subject(&NotificationType::Welcome, None), "Welcome");
        assert_eq!(email_subject(&NotificationType::Welcome, Some("es")), "Bienvenido");
        assert_eq!(
            email_subject(&NotificationType::PasswordReset, Some("fr")),
            "Réinitialiser le mot de passe"
        );
        assert_eq!(
            email_subject(&NotificationType::AccountUpdate, Some("de")),
            "Kontoaktualisierung"
        );
    }

    #[test]
    fn email_body_is_localized() {
        assert_eq!(
            email_body(&NotificationType::Welcome, None, "Alice"),
            "Hello Alice, thanks for signing up!"
        );
        assert_eq!(
            email_body(&NotificationType::Welcome, Some("es"), "Alice"),
            "Hola Alice, ¡gracias por registrarte!"
        );
        assert_eq!(
            email_body(&NotificationType::PasswordReset, Some("fr"), "Alice"),
            "Bonjour Alice, cliquez sur le lien pour réinitialiser votre mot de passe."
        );
        assert_eq!(
            email_body(&NotificationType::AccountUpdate, Some("de"), "Alice"),
            "Hallo Alice, dein Konto wurde aktualisiert."
        );
    }
}