use std::option::Option;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::lib::fast_hash::hash_file;

use super::fs;
use super::Result;

#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct FileEntry {
    pub relative_path: PathBuf,
    pub fast_hash: Option<u128>,
    pub stat_size: u64,
    pub stat_modified: SystemTime,
    pub stat_accessed: SystemTime,
    pub stat_created: SystemTime,
    // in the case of non-duplicate files with the same size and hash, the inode resolves the duplicates
    pub stat_inode: u64,
}

impl FileEntry {
    pub fn new<F: fs::AbstractFs, P1: AsRef<Path>, P2: AsRef<Path>>(fs: &F, base_path: P1, path: P2) -> Result<Self> {
        let absolute_path: PathBuf = fs.canonicalize(path)?;
        let relative_path = absolute_path.strip_prefix(base_path)?;
        // we want to find the relative folder just to make sure that exists, so that we can find it
        // and safely unwrap the option later
        let _ = relative_path.parent().ok_or("error finding relative folder")?;

        let (size, modified, accessed, created, inode) = fs.metadata(&absolute_path)?;

        Ok(Self {
            relative_path: relative_path.to_owned(),
            fast_hash: None,
            stat_size: size,
            stat_modified: modified,
            stat_accessed: accessed,
            stat_created: created,
            stat_inode: inode,
        })
    }

    pub fn absolute_path<P: AsRef<Path>>(&self, base_path: P) -> PathBuf {
        base_path.as_ref().join(self.relative_path.as_path())
    }

    pub fn relative_folder(&self) -> &Path {
        self.relative_path.parent().unwrap()
    }
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    use crate::lib::file_entry::FileEntry;
    use crate::lib::fs::{Path, TestFs};
    use std::time::SystemTime;

    #[test]
    fn test_new_file_entry() {
        let mut test_fs = TestFs::default();
        let base_path = "/somefolder/";
        test_fs.set_cwd(base_path);

        test_fs.add_text_file("/somefolder/filepath", "test");
        let file_hash = FileEntry::new(&test_fs, Path::new("/somefolder/"), Path::new("filepath")).unwrap();

        assert_eq!(file_hash.absolute_path("/somefolder/"), Path::new("/somefolder/filepath"));
        assert_eq!(file_hash.relative_path, Path::new("filepath"));
        assert_eq!(file_hash.relative_folder(), Path::new(""));


        test_fs.add_text_file("/somefolder/subfolder/file", "test");
        let file_hash = FileEntry::new(
            &test_fs,
            Path::new("/somefolder/"),
            Path::new("subfolder/file"),
        ).unwrap();
        assert_eq!(file_hash.absolute_path("/somefolder/"), Path::new("/somefolder/subfolder/file"));
        assert_eq!(file_hash.relative_path, Path::new("subfolder/file"));
        assert_eq!(file_hash.relative_folder(), Path::new("subfolder/"));
    }
}
