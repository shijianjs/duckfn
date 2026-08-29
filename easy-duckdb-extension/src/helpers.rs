use quack_rs::error::ExtensionError;

pub type DuckResult<T> = Result<T, ExtensionError>;
pub fn duck_error(message: impl Into<String>) -> ExtensionError {
    ExtensionError::new(message)
}

pub fn panic_to_string(e: Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = e.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = e.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic".to_string()
    }
}