pub use std::io;
pub use std::path::Path;
use std::path::PathBuf;
use super::Result;
use super::Error;

use mockall::*;
use mockall::predicate::*;
use std::ops::{DerefMut};
use std::borrow::{BorrowMut, Borrow};
use std::cell::UnsafeCell;
use std::fs::Metadata;
use std::time::SystemTime;

pub trait AbstractFs {
    type File: std::io::Read;
    fn open<P: AsRef<Path>>(&self, path: P) -> Result<Self::File>;
    fn canonicalize<P: AsRef<Path>>(&self, path: P) -> Result<PathBuf>;
    // size, modified, accessed, created, inode
    fn metadata<P: AsRef<Path>>(&self, path: P) -> Result<(u64, SystemTime, SystemTime, SystemTime, u64)>;
}

cfg_if::cfg_if! {
    if #[cfg(not(test))] {
        pub use RealFs as Fs;
    } else {
        pub use TestFs as Fs;
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////


#[derive(Debug, Default)]
pub struct RealFs {}

impl RealFs {}


impl AbstractFs for RealFs {
    type File = std::fs::File;

    fn open<P: AsRef<Path>>(&self, path: P) -> Result<Self::File> {
        Self::File::open(path).map_err(|e| e.into())
    }

    fn canonicalize<P: AsRef<Path>>(&self, path: P) -> Result<PathBuf> {
        std::fs::canonicalize(path).map_err(|e| e.into())
    }

    fn metadata<P: AsRef<Path>>(&self, path: P) -> Result<(u64, SystemTime, SystemTime, SystemTime, u64)> {
        let m = std::fs::metadata(path)?;
        if !m.is_file() {
            return Err("path is not a file".into());
        }
        use std::os::linux::fs::MetadataExt;
        Ok((m.len(), m.modified()?, m.accessed()?, m.created()?, m.st_ino()))
    }
}


////////////////////////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
#[derive(Debug, Default)]
pub struct TestFs<'a> {
    filedata: std::collections::HashMap<&'a str, &'a [u8]>,
    inodes: std::collections::HashMap<&'a str, u64>,
    cwd: PathBuf,
    count: UnsafeCell<i64>,
}


#[cfg(test)]
impl<'a> TestFs<'a> {
    #[allow(dead_code)]
    pub fn with_files(files: &[(&'static str, &'static str)]) -> TestFs<'static> {
        TestFs {
            filedata: files.to_owned().iter()
                .map(|(a, b)| (a.to_owned(), b.to_owned().as_bytes()))
                .collect(),
            // default to everything is a unique inode
            inodes: files.to_owned().iter()
                .enumerate()
                .map(|(i, (a, _))| (a.to_owned(), (i+1) as u64))
                .collect(),
            cwd: PathBuf::from("/"),
            count: UnsafeCell::new(0),
        }
    }

    fn next_inode(&self) -> u64 {
        self.inodes.values().max()
            .cloned()
            .unwrap_or(1u64) + 1
    }

    pub fn set_cwd<P: AsRef<Path>>(&mut self, path: P) {
        self.cwd = path.as_ref().to_owned();
    }

    pub fn add_text_file(&mut self, filename: &'static str, filedata: &'static str) {
        self.filedata.insert(filename, filedata.as_bytes());
        self.inodes.insert(filename, self.next_inode());
    }

    pub fn add_binary_file(&mut self, filename: &'static str, filedata: &'static [u8]) {
        self.filedata.insert(filename, filedata);
        self.inodes.insert(filename, self.next_inode());
    }
}

#[cfg(test)]
impl<'a> AbstractFs for TestFs<'a> {
    type File = std::io::Cursor<Box<[u8]>>;

    fn open<P: AsRef<Path>>(&self, path: P) -> Result<Self::File> {
        use std::io::ErrorKind;
        match self.filedata.get(&path.as_ref().to_string_lossy().as_ref()) {
            None => Err("File not found".into()),
            Some(s) => Ok(std::io::Cursor::new(s.to_vec().into_boxed_slice())),
        }
    }

    fn canonicalize<P: AsRef<Path>>(&self, path: P) -> Result<PathBuf> {
        unsafe {
            *self.count.get() += 1;
        }
        // TODO what should we even do here?
        if path.as_ref().has_root() {
            Ok(path.as_ref().to_owned())
        } else {
            Ok(self.cwd.join(path))
        }
    }

    // size, modified, accessed, created, inode
    fn metadata<P: AsRef<Path>>(&self, path: P) -> Result<(u64, SystemTime, SystemTime, SystemTime, u64)> {
        let path_str = path.as_ref().to_string_lossy();
        let buf = self.filedata.get(path_str.as_ref())
            .ok_or(Error::Generic(format!("file {:?} not found", path_str)))?;
        let inode = self.inodes.get(path_str.as_ref()).ok_or(Error::Generic(format!("file {:?} not found", path_str)))?;
        Ok((
            buf.len() as u64,
            SystemTime::now(),
            SystemTime::now(),
            SystemTime::now(),
            inode.clone(),
        ))
    }
}
