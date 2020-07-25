use std::option::Option;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::Deserialize;
use serde::Serialize;

use super::fs;
use super::Result;

#[derive(Deserialize, Serialize, Debug, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct FileEntry {
    pub relative_path: PathBuf,
    pub fast_hash: Option<u128>,
    pub stat_size: u64,
    #[serde(with = "humantime_serde")]
    pub stat_modified: SystemTime,
    #[serde(with = "humantime_serde")]
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
            stat_created: created,
            stat_inode: inode,
        })
    }

    pub fn reload_from_disk<F: fs::AbstractFs, P1: AsRef<Path>>(&self, fs: &F, base_path: P1) -> Result<Self> {
        let mut new_entry = FileEntry::new(fs, &base_path, self.absolute_path(&base_path))?;
        if self.eq_except_hash(&new_entry) {
            new_entry.fast_hash = self.fast_hash;
        }
        Ok(new_entry)
    }

    pub fn agrees_with_disk<F: fs::AbstractFs, P1: AsRef<Path>>(&self, fs: &F, base_path: P1) -> Result<bool> {
        let new_entry = FileEntry::new(fs, &base_path, self.absolute_path(&base_path))?;
        Ok(self.eq_except_hash(&new_entry))
    }

    pub fn absolute_path<P: AsRef<Path>>(&self, base_path: P) -> PathBuf {
        base_path.as_ref().join(self.relative_path.as_path())
    }

    pub fn relative_folder(&self) -> &Path {
        self.relative_path.parent().unwrap()
    }

    pub fn eq_except_hash(&self, other: &Self) -> bool {
        (
            &self.relative_path,
            &self.stat_size,
            &self.stat_modified,
            &self.stat_created,
            &self.stat_inode
        ) == (
            &other.relative_path,
            &other.stat_size,
            &other.stat_modified,
            &other.stat_created,
            &other.stat_inode
        )
    }
}

#[cfg(test)]
mod test {
    use std::path::Path;

    use crate::lib::file_entry::FileEntry;
    use crate::lib::fs::TestFs;

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
