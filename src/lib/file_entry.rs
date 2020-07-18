use std::path::{Path, PathBuf};
use std::option::Option;
use std::time::SystemTime;
use std::io::Result;

use super::fs;
use crate::lib::fast_hash::hash_file;
use crate::lib::fs::io::{Error, ErrorKind};


#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct FileEntry<'a> {
    pub relative_path: PathBuf,
    pub relative_folder: PathBuf,
    pub absolute_path: PathBuf,
    pub fast_hash: Option<u128>,
    // TODO: make this a numeric?
    pub sha256_hash: Option<&'a str>,
    pub stat_size: Option<u64>,
    // unique_id is used to disambiguate files with the same fast_hash
    pub unique_id: Option<u64>,
    pub stat_modified: Option<SystemTime>,
    pub stat_accessed: Option<SystemTime>,
    pub stat_created: Option<SystemTime>,
    pub stat_inode: Option<u64>,
}

impl<'a> FileEntry<'a> {
    // TODO: make this accept a different kind of path type, like the generic ref one maybe
    pub fn new<F: fs::AbstractFs>(fs: &F, base_path: &Path, path: &Path) -> Result<Self> {
        // TODO: maybe switch to a different kind of error type?
        let absolute_path: PathBuf = match fs.canonicalize(path) {
            Ok(p) => p,
            Err(_) => return Err(Error::new(ErrorKind::InvalidInput, "error finding absolute path")),
        };
        println!("{:?}", absolute_path);
        let relative_path = match absolute_path.strip_prefix(base_path) {
            Ok(p) => p,
            Err(_) => return Err(Error::new(ErrorKind::InvalidInput, "error making relative path")),
        };
        let relative_folder = match relative_path.parent() {
            Some(p) => p,
            None => return Err(Error::new(ErrorKind::InvalidInput, "error finding relative folder")),
        };
        Ok(Self {
            relative_path: relative_path.to_owned(),
            relative_folder: relative_folder.to_owned(),
            absolute_path: absolute_path.clone(),
            fast_hash: None,
            sha256_hash: None,
            stat_size: None,
            unique_id: None,
            stat_modified: None,
            stat_accessed: None,
            stat_created: None,
            stat_inode: None,
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
    use crate::lib::fs::{TestFs, Path};
    use crate::lib::file_entry::FileEntry;
    use std::path::PathBuf;

    #[test]
    fn test_new_file_entry() {
        let mut test_fs = TestFs::default();
        test_fs.add_text_file("/somefolder/filepath", "test");
        test_fs.set_cwd("/somefolder/");

        let file_hash = FileEntry::new(&test_fs, Path::new("/somefolder/"), Path::new("filepath")).unwrap();
        assert_eq!(file_hash.absolute_path, Path::new("/somefolder/filepath"));
        assert_eq!(file_hash.relative_path, Path::new("filepath"));
        assert_eq!(file_hash.relative_folder, Path::new(""));


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
            stat_size: None,
            unique_id: None,
            stat_modified: None,
            stat_accessed: None,
            stat_created: None,
            stat_inode: None,
        };

        assert_eq!(file_hash.fast_hash, None);
        let file_hash2 = file_hash.add_fast_hash(&test_fs).unwrap();
        assert_eq!(file_hash2.fast_hash, Some(204797213367049729698754624420042367389));
    }
}