use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;
use serde_json::Value;
use thiserror::Error;

/// Unified JSON error body returned by all handlers.
#[derive(Debug, Serialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
    pub details: Option<Value>,
    #[serde(skip)]
    status: StatusCode,
}

impl ApiError {
    pub fn new(status: StatusCode, code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
            details: None,
            status,
        }
    }

    /// Build an `ApiError` from a typed [`ApiErrorKind`], using the kind's
    /// stable string `code` and HTTP status.
    pub fn from_kind(kind: ApiErrorKind, message: impl Into<String>) -> Self {
        Self {
            code: kind.code().to_string(),
            message: message.into(),
            details: None,
            status: kind.status(),
        }
    }
}

/// Typed error kinds used by handlers, replacing ad-hoc string error codes.
///
/// Each variant maps to a stable string `code` (serialised in the JSON body)
/// and an HTTP status. Use [`ApiError::from_kind`] or the
/// `From<ApiErrorKind> for ApiError` impl to build a response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiErrorKind {
    VaultNotFound,
    ShareTokenNotFound,
    InvalidShareToken,
    ShareTokenRevoked,
    ShareTokenExpired,
    ShareTokenWrongPermission,
    TokenVaultMismatch,
    UnsupportedFormat,
    Forbidden,
    InvalidInput,
    Internal,
}

impl ApiErrorKind {
    /// Stable string code emitted in the JSON error body.
    pub fn code(self) -> &'static str {
        match self {
            ApiErrorKind::VaultNotFound => "vault_not_found",
            ApiErrorKind::ShareTokenNotFound => "share_token_not_found",
            ApiErrorKind::InvalidShareToken => "invalid_share_token",
            ApiErrorKind::ShareTokenRevoked => "share_token_revoked",
            ApiErrorKind::ShareTokenExpired => "share_token_expired",
            ApiErrorKind::ShareTokenWrongPermission => "share_token_wrong_permission",
            ApiErrorKind::TokenVaultMismatch => "token_vault_mismatch",
            ApiErrorKind::UnsupportedFormat => "unsupported_format",
            ApiErrorKind::Forbidden => "forbidden",
            ApiErrorKind::InvalidInput => "invalid_input",
            ApiErrorKind::Internal => "internal_error",
        }
    }

    /// HTTP status associated with the kind.
    pub fn status(self) -> StatusCode {
        match self {
            ApiErrorKind::VaultNotFound => StatusCode::NOT_FOUND,
            ApiErrorKind::ShareTokenNotFound => StatusCode::NOT_FOUND,
            ApiErrorKind::InvalidShareToken => StatusCode::BAD_REQUEST,
            ApiErrorKind::ShareTokenRevoked => StatusCode::GONE,
            ApiErrorKind::ShareTokenExpired => StatusCode::GONE,
            ApiErrorKind::ShareTokenWrongPermission => StatusCode::FORBIDDEN,
            ApiErrorKind::TokenVaultMismatch => StatusCode::BAD_REQUEST,
            ApiErrorKind::UnsupportedFormat => StatusCode::UNPROCESSABLE_ENTITY,
            ApiErrorKind::Forbidden => StatusCode::FORBIDDEN,
            ApiErrorKind::InvalidInput => StatusCode::UNPROCESSABLE_ENTITY,
            ApiErrorKind::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<ApiErrorKind> for ApiError {
    fn from(kind: ApiErrorKind) -> Self {
        ApiError::from_kind(kind, kind.code())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let status = self.status;
        (status, Json(self)).into_response()
    }
}

/// Errors produced by the database layer.
///
/// The DB helpers in `db.rs` return `Result<_, AppError>` instead of panicking
/// via `unwrap()`/`expect()`. The variants below are the ones the DB layer can
/// surface:
///
/// * [`AppError::Db`] — a `rusqlite` failure (query, connection, or a corrupted
///   database file). Converted automatically from [`rusqlite::Error`].
/// * [`AppError::Internal`] — a poisoned mutex/lock, or any other unexpected
///   internal condition. Converted automatically from [`std::sync::PoisonError`].
/// * [`AppError::NotFound`] — a lookup that matched no row.
/// * [`AppError::InvalidInput`] — a value that failed validation before hitting
///   the database.
#[derive(Debug, Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("not found")]
    NotFound,
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("2FA required")]
    TwoFactorRequired,
    #[error("2FA not enabled")]
    TwoFactorNotEnabled,
    #[error("unauthorized: {0}")]
    Unauthorized(String),
    #[error("forbidden: {0}")]
    Forbidden(String),
    #[error("Soroban RPC error: {0}")]
    Rpc(String),
    #[error("Soroban RPC is not configured")]
    RpcUnavailable,
    #[error("email delivery failed: {0}")]
    EmailDelivery(String),
    #[error("sponsor account has insufficient balance: {0}")]
    SponsorInsufficientBalance(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let (status, code) = match &self {
            AppError::NotFound => (StatusCode::NOT_FOUND, "not_found"),
            AppError::InvalidInput(_) => (StatusCode::UNPROCESSABLE_ENTITY, "invalid_input"),
            AppError::Db(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal_error"),
            AppError::TwoFactorRequired => (StatusCode::UNAUTHORIZED, "two_factor_required"),
            AppError::TwoFactorNotEnabled => (StatusCode::BAD_REQUEST, "two_factor_not_enabled"),
            AppError::Unauthorized(_) => (StatusCode::UNAUTHORIZED, "unauthorized"),
            AppError::Forbidden(_) => (StatusCode::FORBIDDEN, "forbidden"),
            AppError::Rpc(_) => (StatusCode::BAD_GATEWAY, "rpc_error"),
            AppError::RpcUnavailable => (StatusCode::SERVICE_UNAVAILABLE, "rpc_unavailable"),
            AppError::EmailDelivery(_) => (StatusCode::BAD_GATEWAY, "email_delivery_failed"),
            AppError::SponsorInsufficientBalance(_) => {
                (StatusCode::PAYMENT_REQUIRED, "sponsor_insufficient_balance")
            }
            AppError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal_error"),
        };
        ApiError::new(status, code, self.to_string()).into_response()
    }
}

/// Convert a poisoned lock error into a typed internal error so handlers can
/// propagate it with `?` instead of panicking via `unwrap()`/`expect()`.
impl<T> From<std::sync::PoisonError<T>> for AppError {
    fn from(_: std::sync::PoisonError<T>) -> Self {
        AppError::Internal("lock poisoned".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poisoned_lock_maps_to_internal_error() {
        let err: AppError = std::sync::PoisonError::new(()).into();
        assert!(matches!(err, AppError::Internal(_)));
    }

    #[test]
    fn internal_error_maps_to_500() {
        let response = AppError::Internal("boom".to_string()).into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn invalid_input_maps_to_422() {
        let response = AppError::InvalidInput("bad".to_string()).into_response();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[test]
    fn not_found_maps_to_404() {
        let response = AppError::NotFound.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn sponsor_insufficient_balance_maps_to_402() {
        let err = AppError::SponsorInsufficientBalance("balance below fee".to_string());
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::PAYMENT_REQUIRED);
    }

    #[test]
    fn api_error_serializes_to_code_message_details() {
        let err = ApiError::from_kind(ApiErrorKind::VaultNotFound, "vault missing");
        let value = serde_json::to_value(&err).expect("serialize");
        assert_eq!(value["code"], "vault_not_found");
        assert_eq!(value["message"], "vault missing");
        assert!(value.get("details").is_some());
        assert!(value["details"].is_null());
        assert!(value.get("status").is_none());
    }

    #[test]
    fn api_error_kind_statuses_map_correctly() {
        assert_eq!(
            ApiError::from_kind(ApiErrorKind::VaultNotFound, "x").into_response().status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            ApiError::from_kind(ApiErrorKind::ShareTokenNotFound, "x").into_response().status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            ApiError::from_kind(ApiErrorKind::Forbidden, "x").into_response().status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            ApiError::from_kind(ApiErrorKind::InvalidShareToken, "x").into_response().status(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            ApiError::from_kind(ApiErrorKind::InvalidInput, "x").into_response().status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
        assert_eq!(
            ApiError::from_kind(ApiErrorKind::Internal, "x").into_response().status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[test]
    fn api_error_kind_codes_are_stable() {
        assert_eq!(ApiErrorKind::VaultNotFound.code(), "vault_not_found");
        assert_eq!(ApiErrorKind::InvalidShareToken.code(), "invalid_share_token");
        assert_eq!(ApiErrorKind::UnsupportedFormat.code(), "unsupported_format");
        assert_eq!(ApiErrorKind::Forbidden.code(), "forbidden");
        assert_eq!(ApiErrorKind::InvalidInput.code(), "invalid_input");
        assert_eq!(ApiErrorKind::Internal.code(), "internal_error");
    }

    #[test]
    fn api_error_kind_converts_into_api_error() {
        let err: ApiError = ApiErrorKind::Forbidden.into();
        assert_eq!(err.code, "forbidden");
        assert_eq!(err.into_response().status(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn db_error_maps_to_500() {
        let err = AppError::from(rusqlite::Error::InvalidQuery);
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn corrupted_db_file_surfaces_as_db_error() {
        // A file that is not a valid SQLite database makes `open` fail; the DB
        // layer must surface this as `AppError::Db` rather than panicking.
        let dir = std::env::temp_dir().join(format!(
            "handsoff_corrupt_db_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let path = dir.join("corrupted.db");
        std::fs::write(&path, b"this is not a valid sqlite database").expect("write corrupt db");

        let result = rusqlite::Connection::open(&path)
            .and_then(|conn| conn.query_row("SELECT count(*) FROM sqlite_master", [], |row| {
                row.get::<_, i64>(0)
            }));

        let err = AppError::from(result.expect_err("corrupted db should error"));
        assert!(matches!(err, AppError::Db(_)));
        assert_eq!(err.into_response().status(), StatusCode::INTERNAL_SERVER_ERROR);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
