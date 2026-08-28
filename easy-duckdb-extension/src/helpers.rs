use quack_rs::error::ExtensionError;

pub type DuckResult<T> = Result<T, ExtensionError>;
