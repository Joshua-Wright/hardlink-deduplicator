use std::path::Path;
use std::option::Option;
use std::time::SystemTime;
use std::io::Result;

use super::fs;
use crate::lib::fast_hash::hash_file;


#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct FileHash<'a> {
    relative_path: &'a Path,
    relative_folder: &'a Path,
    absolute_path: &'a Path,
    fast_hash: Option<u128>,
    sha256_hash: Option<&'a str>,
    // TODO: make this a numeric?
    stat_size: Option<u64>,
    stat_modified: Option<SystemTime>,
    stat_accessed: Option<SystemTime>,
    stat_created: Option<SystemTime>,
    stat_inode: Option<u64>,
}

impl<'a> FileHash<'a> {
    // pub fn new(base_path: Path, path: Path) -> Self {
    //     // TODO abstract this stuff, through fs shim
    // }
    pub fn add_hashes<F: fs::Fs>(&self, fs: &F) -> Result<Self> {
        let hash = hash_file(fs, self.absolute_path)?;
        Ok(Self {
            fast_hash: Some(hash),
            ..*self
        })
    }
}

#[cfg(test)]
mod test {
    use crate::lib::fs::{TestFs, Path};
    use crate::lib::file_entry::FileHash;

    #[test]
    fn test_add_hash() {
        let mut test_fs = TestFs::default();
        test_fs.add_text_file("/somefolder/filepath", "test");

        let file_hash = FileHash {
            relative_path: Path::new("filepath"),
            relative_folder: Path::new("."),
            absolute_path: Path::new("/somefolder/filepath"),
            fast_hash: None,
            sha256_hash: None,
            stat_size: None,
            stat_modified: None,
            stat_accessed: None,
            stat_created: None,
            stat_inode: None,
        };

        assert_eq!(file_hash.fast_hash, None);
        let file_hash2 = file_hash.add_hashes(&test_fs).unwrap();
        assert_eq!(file_hash2.fast_hash, Some(204797213367049729698754624420042367389));
    }
}