//! Error types for Armoury Crate Linux

use std::fmt;

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Dbus(zbus::Error),
    Config(String),
    Monitoring(String),
    Slint(slint::PlatformError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "IO error: {}", e),
            Error::Dbus(e) => write!(f, "D-Bus error: {}", e),
            Error::Config(e) => write!(f, "Config error: {}", e),
            Error::Monitoring(e) => write!(f, "Monitoring error: {}", e),
            Error::Slint(e) => write!(f, "Slint error: {}", e),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<zbus::Error> for Error {
    fn from(e: zbus::Error) -> Self {
        Error::Dbus(e)
    }
}

impl From<slint::PlatformError> for Error {
    fn from(e: slint::PlatformError) -> Self {
        Error::Slint(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
