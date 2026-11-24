pub mod email;
pub mod sms;
pub mod webhook;

pub use email::*;
pub use sms::*;
pub use webhook::*;

use crate::models::vault::Vault;
use crate::error::AppError;
use serde::{Deserialize, Serialize};

/// A vault template definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub fields: Vec<VaultTemplateField>,
}

/// A single field within a vault template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultTemplateField {
    pub name: String,
    pub field_type: String,
    pub required: bool,
    pub default_value: Option<String>,
}

/// Returns the list of available vault templates.
pub fn get_vault_templates() -> Vec<VaultTemplate> {
    vec![
        VaultTemplate {
            id: "personal".to_string(),
            name: "Personal".to_string(),
            description: "A general-purpose vault for personal credentials.".to_string(),
            category: "general".to_string(),
            fields: vec![
                VaultTemplateField {
                    name: "username".to_string(),
                    field_type: "text".to_string(),
                    required: true,
                    default_value: None,
                },
                VaultTemplateField {
                    name: "password".to_string(),
                    field_type: "password".to_string(),
                    required: true,
                    default_value: None,
                },
                VaultTemplateField {
                    name: "notes".to_string(),
                    field_type: "text".to_string(),
                    required: false,
                    default_value: None,
                },
            ],
        },
        VaultTemplate {
            id: "banking".to_string(),
            name: "Banking".to_string(),
            description: "A vault tailored for banking and financial credentials.".to_string(),
            category: "finance".to_string(),
            fields: vec![
                VaultTemplateField {
                    name: "account_number".to_string(),
                    field_type: "text".to_string(),
                    required: true,
                    default_value: None,
                },
                VaultTemplateField {
                    name: "pin".to_string(),
                    field_type: "password".to_string(),
                    required: true,
                    default_value: None,
                },
                VaultTemplateField {
                    name: "routing_number".to_string(),
                    field_type: "text".to_string(),
                    required: false,
                    default_value: None,
                },
            ],
        },
        VaultTemplate {
            id: "social".to_string(),
            name: "Social Media".to_string(),
            description: "A vault for social media accounts.".to_string(),
            category: "general".to_string(),
            fields: vec![
                VaultTemplateField {
                    name: "username".to_string(),
                    field_type: "text".to_string(),
                    required: true,
                    default_value: None,
                },
                VaultTemplateField {
                    name: "password".to_string(),
                    field_type: "password".to_string(),
                    required: true,
                    default_value: None,
                },
                VaultTemplateField {
                    name: "email".to_string(),
                    field_type: "email".to_string(),
                    required: false,
                    default_value: None,
                },
            ],
        },
    ]
}

/// Creates a new vault from the given template.
pub fn create_vault_from_template(
    template_id: &str,
    owner_id: &str,
) -> Result<Vault, AppError> {
    let template = get_vault_templates()
        .into_iter()
        .find(|t| t.id == template_id)
        .ok_or_else(|| AppError::NotFound(format!("Vault template '{}' not found", template_id)))?;

    let vault = Vault {
        id: uuid::Uuid::new_v4().to_string(),
        name: template.name.clone(),
        description: Some(template.description.clone()),
        owner_id: owner_id.to_string(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    Ok(vault)
}