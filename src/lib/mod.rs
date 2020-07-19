pub mod files_index;
pub mod fs;
pub mod fast_hash;
pub mod file_entry;


use std::io::Result as IOResult;
use std::path::StripPrefixError;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Generic(String),
    IO(std::io::Error),
    StripPrefixError(std::path::StripPrefixError),
}

impl From<String> for Error {
    fn from(s: String) -> Self {
        Error::Generic(s)
    }
}

impl From<&str> for Error {
    fn from(s: &str) -> Self {
        Error::Generic(s.to_owned())
    }
}


impl From<std::path::StripPrefixError> for Error {
    fn from(e: StripPrefixError) -> Self {
        Error::StripPrefixError(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::IO(e)
    }
}



// impl<T> Result<T> {
//     fn from_option(o: Option<T>, msg: String) -> Self {
//         o.ok_or(msg)
//     }
// }
// impl<T> From<IOResult<T>> for Result<T> {
//     fn from(r: IOResult<T>) -> Self {
//         r.map_err(|e| e.to_string())
//     }
// }
