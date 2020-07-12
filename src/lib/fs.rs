pub use std::io;
pub use std::io::Result;
pub use std::path::Path;
use std::path::PathBuf;

use mockall::*;
use mockall::predicate::*;

pub trait Fs {
    type File: std::io::Read;
    fn open<P: AsRef<Path>>(&self, path: P) -> Result<Self::File>;
    fn canonicalize<P: AsRef<Path>>(&self, path: P) -> Result<PathBuf>;
}

////////////////////////////////////////////////////////////////////////////////////////////////////


#[derive(Debug, Default)]
pub struct RealFs {}

impl RealFs {}


impl Fs for RealFs {
    type File = std::fs::File;

    fn open<P: AsRef<Path>>(&self, path: P) -> Result<Self::File> {
        Self::File::open(path)
    }

    fn canonicalize<P: AsRef<Path>>(&self, path: P) -> Result<PathBuf> {
        std::fs::canonicalize(path)
    }
}


////////////////////////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
#[derive(Debug, Default)]
pub struct TestFs<'a> {
    filedata: std::collections::HashMap<&'a str, &'a [u8]>,
    cwd: PathBuf,
}


#[cfg(test)]
impl TestFs<'static> {
    #[allow(dead_code)]
    pub fn with_files(files: Vec<(&'static str, &'static str)>) -> TestFs<'static> {
        use std::collections::HashMap;
        use std::iter::FromIterator;
        TestFs {
            filedata: HashMap::from_iter(files.iter()
                .map(|(a, b)| (a.to_owned(), b.to_owned().as_bytes())
                )),
            cwd: PathBuf::from("/"),
        }
    }

    pub fn set_cwd<P: AsRef<Path>>(&mut self, path: P) {
        self.cwd = path.as_ref().to_owned();
    }

    pub fn add_text_file(&mut self, filename: &'static str, filedata: &'static str) {
        self.filedata.insert(filename, filedata.as_bytes());
    }

    pub fn add_binary_file(&mut self, filename: &'static str, filedata: &'static [u8]) {
        self.filedata.insert(filename, filedata);
    }
}

#[cfg(test)]
impl Fs for TestFs<'static> {
    type File = std::io::Cursor<&'static [u8]>;

    fn open<P: AsRef<Path>>(&self, path: P) -> Result<Self::File> {
        use std::io::ErrorKind;
        match self.filedata.get(&path.as_ref().to_string_lossy().as_ref()) {
            None => Err(std::io::Error::new(ErrorKind::NotFound, "File not found")),
            Some(s) => Ok(std::io::Cursor::new(s)),
        }
    }

    fn canonicalize<P: AsRef<Path>>(&self, path: P) -> Result<PathBuf> {
        // TODO what should we even do here?
        if path.as_ref().has_root() {
            Ok(path.as_ref().to_owned())
        } else {
            Ok(self.cwd.join(path))
        }
    }
}
