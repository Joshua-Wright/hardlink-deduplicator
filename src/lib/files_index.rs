use std::borrow::Borrow;
use std::collections::hash_map::RandomState;
use std::collections::HashMap;
use std::option::Option;
use std::path::{Path, PathBuf};

use super::file_entry::FileEntry;
use crate::lib::fs::AbstractFs;
use crate::lib::Result;

#[derive(Ord, PartialOrd, Eq, PartialEq, Hash, Debug)]
enum FileEntryKey {
    SizeOnly { size: u64 },
    FullKey { size: u64, fast_hash: u128, unique_id: u64 },
}


fn group_by_size(entries: &[FileEntry]) -> HashMap<u64, Vec<usize>> {
    let mut out: HashMap<u64, Vec<usize>, RandomState> = HashMap::new();
    for (i, e) in entries.iter().enumerate() {
        let size = e.stat_size;
        out.entry(size)
            .or_insert_with(|| vec![i]);
    }
    out
}

// invariant: all files in the files index are already deduplicated: they are either unique or they
// have the same unique_id
#[derive(Default, Debug, Clone)]
pub struct FilesIndex {
    // TODO: put base path here instead?
    entries: Vec<FileEntry>,
    by_path: HashMap<PathBuf, usize>,
    by_size: HashMap<u64, Vec<usize>>,
}

impl FilesIndex {
    fn from_entries(entries: &[FileEntry]) -> Self {
        let by_path = entries.iter()
            .enumerate()
            .map(|(i, e)| (e.relative_path.to_owned(), i))
            .collect();

        Self {
            entries: entries.to_vec(),
            by_path,
            by_size: group_by_size(entries),
        }
    }


    pub fn get_by_path<'a, P: AsRef<Path>>(&'a self, path: &P) -> Option<&'a FileEntry> {
        self.by_path.get(path.as_ref())
            .map(|&i| { self.entries[i].borrow() })
    }

    // pub fn add_file<'a, Fs: AbstractFs, P: AsRef<Path>>(&'a mut self, fs: &Fs, base_path: &P, path: &P) -> Result<&'a FileEntry> {
    pub fn add_file<'a, Fs: AbstractFs>(&'a mut self, fs: &Fs, base_path: &Path, path: &Path) -> Result<&'a FileEntry> {
        // TODO: probably shouldn't push this until we have deduplicated the file correctly, right? in order to be error tolerant
        self.entries.push(FileEntry::new(fs, base_path.as_ref(), path.as_ref())?);
        let mut entry = self.entries.last().unwrap();
        let entry_index = self.entries.len() - 1;
        if let Some(xs) = self.by_size.get_mut(&entry.stat_size) {
            xs.push(entry_index);
            // TODO: deduplicate this file
            unimplemented!()
        } else {
            self.by_size.entry(entry.stat_size).or_insert_with(|| vec![entry_index])
        };

        self.by_path.insert(entry.relative_path.clone(), entry_index);

        Ok(entry)
    }
}


#[cfg(test)]
mod test {
    use std::path::Path;

    use crate::lib::files_index::FilesIndex;
    use crate::lib::fs::TestFs;
    use std::borrow::Borrow;

    #[test]
    pub fn test_construct() {
        let mut test_fs = TestFs::default();
        test_fs.add_text_file("/somefolder/filepath", "test");
        let base_path = Path::new("/somefolder/");
        test_fs.set_cwd(base_path);

        let file_entries = vec![
            test_fs.new_file_entry("/somefolder/asdf", "test"),
            test_fs.new_file_entry("/somefolder/asdf2", "asdf"),
            test_fs.new_file_entry("/somefolder/newfile", "newf"),
        ];

        let index = FilesIndex::from_entries(&file_entries);
        assert_eq!(index.entries.len(), 3);
        assert_eq!(index.by_path.len(), 3);
        assert_eq!(index.by_size.len(), 1);

        assert_eq!(index.get_by_path(&"asdf").unwrap(), &file_entries[0]);
        assert_eq!(index.get_by_path(&"asdf2").unwrap(), &file_entries[1]);
        assert_eq!(index.get_by_path(&"newfile").unwrap(), &file_entries[2]);
    }


    #[test]
    pub fn test_add_unique_file() {
        let mut test_fs = TestFs::default();
        let base_path = Path::new("/somefolder/");
        test_fs.set_cwd(base_path);

        let mut index = FilesIndex::default();

        let f1 = test_fs.new_file_entry("/somefolder/asdf", "test");
        index.add_file(&test_fs, base_path.borrow(), f1.relative_path.as_path()).unwrap();
        assert_eq!(index.get_by_path(&f1.relative_path.as_path()).unwrap(), &f1);
        assert_eq!(index.entries.len(), 1);
        assert_eq!(index.by_path.len(), 1);
        assert_eq!(index.by_size.len(), 1);


        let f2 = test_fs.new_file_entry("/somefolder/asdfasdf", "testasdf");
        index.add_file(&test_fs, base_path.borrow(), f2.relative_path.as_path()).unwrap();
        assert_eq!(index.get_by_path(&f2.relative_path.as_path()).unwrap(), &f2);
        assert_eq!(index.entries.len(), 2);
        assert_eq!(index.by_path.len(), 2);
        assert_eq!(index.by_size.len(), 2);
    }

    #[test]
    #[should_panic] // TODO: implement deduplication
    pub fn test_add_duplicate_file() {
        let mut test_fs = TestFs::default();
        let base_path = Path::new("/somefolder/");
        test_fs.set_cwd(base_path);

        let mut index = FilesIndex::default();

        let f1 = test_fs.new_file_entry("/somefolder/test", "test");
        let f2 = test_fs.new_file_entry("/somefolder/asdf", "asdf");
        index.add_file(&test_fs, base_path.borrow(), f1.relative_path.as_path()).unwrap();
        index.add_file(&test_fs, base_path.borrow(), f2.relative_path.as_path()).unwrap();
    }
}


