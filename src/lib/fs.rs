pub use std::io;
pub use std::io::Result;
pub use std::path::Path;

pub trait Fs {
    type File: std::io::Read;
    fn open<P: AsRef<Path>>(&self, path: P) -> Result<Self::File>;
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
}


////////////////////////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
#[derive(Debug, Default)]
pub struct TestFs<'a> {
    filedata: std::collections::HashMap<&'a str, &'a [u8]>
}

#[cfg(test)]
impl<'a> TestFs<'a> {
    #[allow(dead_code)]
    pub fn with_files(files: Vec<(&'static str, &'static str)>) -> TestFs<'a> {
        use std::collections::HashMap;
        use std::iter::FromIterator;
        TestFs {
            filedata: HashMap::from_iter(files.iter()
                .map(|(a, b)| (a.to_owned(), b.to_owned().as_bytes())
                ))
        }
    }

    pub fn add_text_file(&mut self, filename: &'a str, filedata: &'a str) {
        self.filedata.insert(filename, filedata.as_bytes());
    }

    pub fn add_binary_file(&mut self, filename: &'a str, filedata: &'a [u8]) {
        self.filedata.insert(filename, filedata);
    }
}

#[cfg(test)]
impl<'a> Fs for TestFs<'a> {
    type File = std::io::Cursor<&'a [u8]>;

    fn open<P: AsRef<Path>>(&self, path: P) -> Result<Self::File> {
        use std::io::ErrorKind;
        match self.filedata.get(&path.as_ref().to_string_lossy().as_ref()) {
            None => Err(std::io::Error::new(ErrorKind::NotFound, "File not found")),
            Some(s) => Ok(std::io::Cursor::new(s)),
        }
    }
}
