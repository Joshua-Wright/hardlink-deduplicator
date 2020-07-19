pub use std::io;
pub use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;

use super::Result;

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

cfg_if::cfg_if! {
    if #[cfg(test)] {
        use std::cell::UnsafeCell;
        use std::ops::Deref;
        use super::Error;
    }
}


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


#[cfg(test)]
#[derive(Debug, Default)]
pub struct TestFs {
    filedata: std::collections::HashMap<String, Vec<u8>>,
    inodes: std::collections::HashMap<String, u64>,
    pub cwd: PathBuf,
    // TODO: turn this into a function call log or something like that
    count: UnsafeCell<i64>,
}

#[cfg(test)]
impl TestFs {
    #[allow(dead_code)]
    pub fn with_files(files: &[(&str, &str)]) -> TestFs {
        TestFs {
            filedata: files.to_owned().iter()
                .map(|(a, b)| (a.deref().to_owned(), b.to_owned().as_bytes().to_vec()))
                .collect(),
            // default to everything is a unique inode
            inodes: files.to_owned().iter()
                .enumerate()
                .map(|(i, (a, _))| (a.deref().to_owned(), (i + 1) as u64))
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

    pub fn add_text_file(&mut self, filename: &str, filedata: &str) {
        self.filedata.insert(filename.to_owned(), filedata.as_bytes().to_vec());
        self.inodes.insert(filename.to_owned(), self.next_inode());
    }

    pub fn add_binary_file(&mut self, filename: &str, filedata: &[u8]) {
        self.filedata.insert(filename.to_owned(), filedata.to_vec());
        self.inodes.insert(filename.to_owned(), self.next_inode());
    }

    pub fn new_file_entry<'b>(&mut self, path: &str, filedata: &str) -> super::file_entry::FileEntry<'b> {
        self.add_text_file(path, filedata);
        super::file_entry::FileEntry::new(self, &self.cwd, Path::new(path)).unwrap()
    }
}

#[cfg(test)]
impl AbstractFs for TestFs {
    type File = std::io::Cursor<Box<[u8]>>;

    fn open<P: AsRef<Path>>(&self, path: P) -> Result<Self::File> {
        match self.filedata.get(&path.as_ref().to_string_lossy().to_string()) {
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
