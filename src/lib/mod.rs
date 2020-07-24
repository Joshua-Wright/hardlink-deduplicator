pub mod files_index;
pub mod fs;
pub mod fast_hash;
pub mod file_entry;


pub type Result<T> = std::result::Result<T, Error>;

// use std::backtrace::Backtrace;
extern crate backtrace;
use backtrace::Backtrace;


#[derive(Debug)]
pub enum Error {
    Generic(Backtrace, String),
    IO(Backtrace, std::io::Error),
    StripPrefixError(Backtrace, std::path::StripPrefixError),
    ReadOnlyFs(),
}

impl From<String> for Error {
    fn from(s: String) -> Self {
        Error::Generic(Backtrace::new(), s)
    }
}

impl From<&str> for Error {
    fn from(s: &str) -> Self {
        Error::Generic(Backtrace::new(), s.to_owned())
    }
}


impl From<std::path::StripPrefixError> for Error {
    fn from(e: std::path::StripPrefixError) -> Self {
        Error::StripPrefixError(Backtrace::new(), e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        // std::backtrace::Backtrace::capture();
        Error::IO(Backtrace::new(), e)
    }
}
