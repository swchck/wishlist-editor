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
    #[error("карточка «{0}» уже существует")]
    AlreadyExists(String),
    #[error("карточка «{0}» не найдена")]
    NotFound(String),
    #[error("ошибка файловой системы: {0}")]
    Io(#[from] std::io::Error),
}

impl AppError {
    fn kind(&self) -> &'static str {
        match self {
            Self::NotAWishlist => "not_a_wishlist",
            Self::NoWorkspace => "no_workspace",
            Self::InvalidSlug(_) => "invalid_slug",
            Self::AlreadyExists(_) => "already_exists",
            Self::NotFound(_) => "not_found",
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

pub type AppResult<T> = Result<T, AppError>;
