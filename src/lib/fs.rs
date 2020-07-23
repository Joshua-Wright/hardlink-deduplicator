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

    fn hard_link<P: AsRef<Path>, Q: AsRef<Path>>(&self, src: P, dst: Q) -> Result<()>;
    fn remove_file<P: AsRef<Path>>(&self, path: P) -> Result<()>;
    fn rename<P: AsRef<Path>, Q: AsRef<Path>>(&self, from: P, to: Q) -> Result<()>;
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
        use std::collections::HashMap;
        use super::Error;
    }
}


#[derive(Debug, Default)]
pub struct RealFs {}

impl RealFs {}


impl AbstractFs for RealFs {
    type File = std::fs::File;

    fn open<P: AsRef<Path>>(&self, path: P) -> Result<Self::File> {
        Self::File::open(path).map_err(Into::into)
    }

    fn canonicalize<P: AsRef<Path>>(&self, path: P) -> Result<PathBuf> {
        std::fs::canonicalize(path).map_err(Into::into)
    }

    fn metadata<P: AsRef<Path>>(&self, path: P) -> Result<(u64, SystemTime, SystemTime, SystemTime, u64)> {
        let m = std::fs::metadata(path)?;
        if !m.is_file() {
            return Err("path is not a file".into());
        }
        use std::os::linux::fs::MetadataExt;
        Ok((m.len(), m.modified()?, m.accessed()?, m.created()?, m.st_ino()))
    }

    fn hard_link<P: AsRef<Path>, Q: AsRef<Path>>(&self, src: P, dst: Q) -> Result<()> {
        std::fs::hard_link(src, dst).map_err(Into::into)
    }
    fn remove_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        std::fs::remove_file(path).map_err(Into::into)
    }
    fn rename<P: AsRef<Path>, Q: AsRef<Path>>(&self, from: P, to: Q) -> Result<()> {
        std::fs::rename(from, to).map_err(Into::into)
    }
}


#[cfg(test)]
#[derive(Debug, Default)]
pub struct TestFs {
    filedata_: UnsafeCell<HashMap<String, Vec<u8>>>,
    inodes_: UnsafeCell<HashMap<String, u64>>,
    pub cwd: PathBuf,
    // TODO: turn this into a function call log or something like that
    count: UnsafeCell<i64>,
}


#[cfg(test)]
fn path_str<P: AsRef<Path>>(path: P) -> String {
    path.as_ref().to_string_lossy().to_string()
}

#[cfg(test)]
impl TestFs {
    #[allow(dead_code)]
    pub fn with_files(files: &[(&str, &str)]) -> TestFs {
        TestFs {
            filedata_: files.to_owned().iter()
                .map(|(a, b)| (a.deref().to_owned(), b.to_owned().as_bytes().to_vec()))
                .collect::<HashMap<String, Vec<u8>>>()
                .into(),
            // default to everything is a unique inode
            inodes_: files.to_owned().iter()
                .enumerate()
                .map(|(i, (a, _))| (a.deref().to_owned(), (i + 1) as u64))
                .collect::<HashMap<String, u64>>()
                .into(),
            cwd: PathBuf::from("/"),
            count: UnsafeCell::new(0),
        }
    }

    // horrible mutation garbage
    fn filedata(&self) -> &HashMap<String, Vec<u8>> {
        unsafe {
            self.filedata_.get().as_ref().unwrap()
        }
    }

    fn inodes(&self) -> &HashMap<String, u64> {
        unsafe {
            self.inodes_.get().as_ref().unwrap()
        }
    }

    fn filedata_mut(&self) -> &mut HashMap<String, Vec<u8>> {
        unsafe {
            self.filedata_.get().as_mut().unwrap()
        }
    }

    fn inodes_mut(&self) -> &mut HashMap<String, u64> {
        unsafe {
            self.inodes_.get().as_mut().unwrap()
        }
    }

    fn next_inode(&self) -> u64 {
        self.inodes().values().max()
            .cloned()
            .unwrap_or(1u64) + 1
    }

    pub fn set_cwd<P: AsRef<Path>>(&mut self, path: P) {
        self.cwd = path.as_ref().to_owned();
    }

    pub fn add_text_file(&mut self, filename: &str, filedata: &str) {
        self.filedata_mut().insert(filename.to_owned(), filedata.as_bytes().to_vec());
        self.inodes_mut().insert(filename.to_owned(), self.next_inode());
    }

    pub fn add_binary_file(&mut self, filename: &str, filedata: &[u8]) {
        self.filedata_mut().insert(filename.to_owned(), filedata.to_vec());
        self.inodes_mut().insert(filename.to_owned(), self.next_inode());
    }

    pub fn new_file_entry(&mut self, path: &str, filedata: &str) -> super::file_entry::FileEntry {
        self.add_text_file(path, filedata);
        super::file_entry::FileEntry::new(self, &self.cwd, Path::new(path)).unwrap()
    }
}

#[cfg(test)]
impl AbstractFs for TestFs {
    type File = std::io::Cursor<Box<[u8]>>;

    fn open<P: AsRef<Path>>(&self, path: P) -> Result<Self::File> {
        match self.filedata().get(&path_str(path)) {
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
        let buf = self.filedata().get(path_str.as_ref())
            .ok_or(Error::from(format!("file {:?} not found", path_str)))?;
        let inode = self.inodes().get(path_str.as_ref()).ok_or(Error::from(format!("file {:?} not found", path_str)))?;
        Ok((
            buf.len() as u64,
            // TODO: fix that
            SystemTime::UNIX_EPOCH,
            SystemTime::UNIX_EPOCH,
            SystemTime::UNIX_EPOCH,
            inode.clone(),
        ))
    }

    fn hard_link<P: AsRef<Path>, Q: AsRef<Path>>(&self, src: P, dst: Q) -> Result<()> {
        if let Some(_) = self.filedata().get(&path_str(&dst)) {
            return Err("dst file exists!".into());
        }
        let file_content = self.filedata().get(&path_str(&src)).ok_or(Error::from("file not found".to_owned()))?;
        let inode = self.inodes().get(&path_str(&src)).ok_or(Error::from("file not found".to_owned()))?;
        self.filedata_mut().insert(path_str(&dst), file_content.clone());
        self.inodes_mut().insert(path_str(&dst), inode.clone());
        Ok(())
    }

    fn remove_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        self.inodes_mut().remove(&path_str(&path)).ok_or(Error::from("file not found"))?;
        self.filedata_mut().remove(&path_str(&path)).ok_or(Error::from("file_not_found"))?;
        Ok(())
    }

    fn rename<P: AsRef<Path>, Q: AsRef<Path>>(&self, from: P, to: Q) -> Result<()> {
        // get the file that we're going to move
        let file_content = self.filedata().get(&path_str(&from)).ok_or(Error::from("file not found".to_owned()))?;
        let inode = self.inodes().get(&path_str(&from)).ok_or(Error::from("file not found".to_owned()))?;

        // if an existing file exists, overwrite it
        if let Some(_) = self.filedata().get(&path_str(&to)) {
            self.inodes_mut().remove(&path_str(&to)).ok_or(Error::from("file not found"))?;
            self.filedata_mut().remove(&path_str(&to)).ok_or(Error::from("file_not_found"))?;
        }

        // remove the old file
        self.inodes_mut().remove(&path_str(&from)).ok_or(Error::from("file not found"))?;
        self.filedata_mut().remove(&path_str(&from)).ok_or(Error::from("file_not_found"))?;

        // insert the new file
        self.filedata_mut().insert(path_str(&to), file_content.clone());
        self.inodes_mut().insert(path_str(&to), inode.clone());
        Ok(())
    }
}
