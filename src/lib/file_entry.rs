use std::option::Option;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::lib::fast_hash::hash_file;

use super::fs;
use super::Result;

#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct FileEntry {
    pub relative_path: PathBuf,
    pub relative_folder: PathBuf,
    pub absolute_path: PathBuf,
    pub fast_hash: Option<u128>,
    // TODO: make this a numeric?
    pub sha256_hash: Option<String>,
    // unique_id is used to disambiguate files with the same fast_hash
    pub unique_id: Option<u64>,
    pub stat_size: u64,
    pub stat_modified: SystemTime,
    pub stat_accessed: SystemTime,
    pub stat_created: SystemTime,
    pub stat_inode: u64,
}

impl FileEntry {
    // TODO: make this accept a different kind of path type, like the generic ref one maybe
    pub fn new<F: fs::AbstractFs>(fs: &F, base_path: &Path, path: &Path) -> Result<Self> {
        let absolute_path: PathBuf = fs.canonicalize(path)?;
        let relative_path = absolute_path.strip_prefix(base_path)?;
        let relative_folder = relative_path.parent().ok_or("error finding relative folder")?;

        let (size, modified, accessed, created, inode) = fs.metadata(&absolute_path)?;

        Ok(Self {
            relative_path: relative_path.to_owned(),
            relative_folder: relative_folder.to_owned(),
            absolute_path: absolute_path.clone(),
            fast_hash: None,
            sha256_hash: None,
            unique_id: None,
            stat_size: size,
            stat_modified: modified,
            stat_accessed: accessed,
            stat_created: created,
            stat_inode: inode,
        })
    }

    pub fn add_fast_hash<F: fs::AbstractFs>(&self, fs: &F) -> Result<Self> {
        let hash = hash_file(fs, &self.absolute_path)?;
        Ok(Self {
            fast_hash: Some(hash),
            ..self.clone()
        })
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
        test_fs.set_cwd("/somefolder/");

        test_fs.add_text_file("/somefolder/filepath", "test");
        let file_hash = FileEntry::new(&test_fs, Path::new("/somefolder/"), Path::new("filepath")).unwrap();

        assert_eq!(file_hash.absolute_path, Path::new("/somefolder/filepath"));
        assert_eq!(file_hash.relative_path, Path::new("filepath"));
        assert_eq!(file_hash.relative_folder, Path::new(""));


        test_fs.add_text_file("/somefolder/subfolder/file", "test");
        let file_hash = FileEntry::new(
            &test_fs,
            Path::new("/somefolder/"),
            Path::new("subfolder/file"),
        ).unwrap();
        assert_eq!(file_hash.absolute_path, Path::new("/somefolder/subfolder/file"));
        assert_eq!(file_hash.relative_path, Path::new("subfolder/file"));
        assert_eq!(file_hash.relative_folder, Path::new("subfolder/"));
    }

    #[test]
    fn test_add_hash() {
        let mut test_fs = TestFs::default();
        test_fs.add_text_file("/somefolder/filepath", "test");

        let file_hash = FileEntry {
            relative_path: PathBuf::from("filepath"),
            relative_folder: PathBuf::from("."),
            absolute_path: PathBuf::from("/somefolder/filepath"),
            fast_hash: None,
            sha256_hash: None,
            unique_id: None,
            stat_size: 1,
            stat_modified: SystemTime::now(),
            stat_accessed: SystemTime::now(),
            stat_created: SystemTime::now(),
            stat_inode: 1,
        };

        assert_eq!(file_hash.fast_hash, None);
        let file_hash2 = file_hash.add_fast_hash(&test_fs).unwrap();
        assert_eq!(file_hash2.fast_hash, Some(204797213367049729698754624420042367389));
    }
}