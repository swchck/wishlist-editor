use serde::Serialize;

/// Error returned to the webview; serialized as `{ kind, message }`.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("папка не похожа на вишлист: нет файла wishlist.config.json")]
    NotAWishlist,
    #[error("вишлист не открыт")]
    NoWorkspace,
    #[error("недопустимое имя карточки «{0}»: только латиница, цифры и дефис")]
    InvalidSlug(String),
    #[error("недопустимое имя репозитория «{0}»")]
    InvalidRepo(String),
    #[error("карточка «{0}» уже существует")]
    AlreadyExists(String),
    #[error("карточка «{0}» не найдена")]
    NotFound(String),
    #[error("нет связи с GitHub")]
    Offline,
    #[error("нужно войти в GitHub")]
    NotSignedIn,
    #[error("GitHub не принял вход — войдите заново")]
    Unauthorized,
    #[error("код входа истёк — начните заново")]
    AuthExpired,
    #[error("вход отменён на GitHub")]
    AuthDenied,
    #[error("вход отменён")]
    AuthCancelled,
    #[error("GitHub ответил ошибкой: {0}")]
    GitHub(String),
    #[error("на GitHub появились новые изменения, пока шла публикация — попробуйте ещё раз")]
    PushRejected,
    #[error("ошибка git: {0}")]
    Git(String),
    #[error("не удалось сохранить вход в связке ключей: {0}")]
    Keyring(String),
    #[error("ошибка файловой системы: {0}")]
    Io(#[from] std::io::Error),
}

impl AppError {
    fn kind(&self) -> &'static str {
        match self {
            Self::NotAWishlist => "not_a_wishlist",
            Self::NoWorkspace => "no_workspace",
            Self::InvalidSlug(_) => "invalid_slug",
            Self::InvalidRepo(_) => "invalid_repo",
            Self::AlreadyExists(_) => "already_exists",
            Self::NotFound(_) => "not_found",
            Self::Offline => "offline",
            Self::NotSignedIn => "not_signed_in",
            Self::Unauthorized => "unauthorized",
            Self::AuthExpired => "auth_expired",
            Self::AuthDenied => "auth_denied",
            Self::AuthCancelled => "auth_cancelled",
            Self::GitHub(_) => "github",
            Self::PushRejected => "push_rejected",
            Self::Git(_) => "git",
            Self::Keyring(_) => "keyring",
            Self::Io(_) => "io",
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut st = s.serialize_struct("AppError", 2)?;
        st.serialize_field("kind", self.kind())?;
        st.serialize_field("message", &self.to_string())?;
        st.end()
    }
}

impl From<git2::Error> for AppError {
    fn from(e: git2::Error) -> Self {
        use git2::{ErrorClass, ErrorCode};
        match (e.class(), e.code()) {
            (_, ErrorCode::Auth) => Self::Unauthorized,
            (ErrorClass::Net | ErrorClass::Ssl | ErrorClass::Http, _) => {
                // libgit2 reports a rejected token as an HTTP 401/403 class error, not Auth
                let msg = e.message();
                if msg.contains("401") || msg.contains("403") || msg.contains("authentication") {
                    Self::Unauthorized
                } else {
                    Self::Offline
                }
            }
            _ => Self::Git(e.message().to_owned()),
        }
    }
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        if e.is_connect() || e.is_timeout() || e.is_request() {
            return Self::Offline;
        }
        if e.status() == Some(reqwest::StatusCode::UNAUTHORIZED) {
            return Self::Unauthorized;
        }
        Self::GitHub(e.to_string())
    }
}

impl From<keyring::Error> for AppError {
    fn from(e: keyring::Error) -> Self {
        Self::Keyring(e.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
