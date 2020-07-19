use std::path::{Path, PathBuf};
use std::option::Option;
use std::collections::HashMap;

use super::file_entry::FileEntry;
use std::borrow::Borrow;
use std::collections::hash_map::RandomState;

#[derive(Ord, PartialOrd, Eq, PartialEq, Hash, Debug)]
enum FileEntryKey {
    SizeOnly { size: u64 },
    FullKey { size: u64, fast_hash: u128, unique_id: u64 },
}


fn group_by_size(entries: &[FileEntry]) -> HashMap<u64, Vec<usize>> {
    let mut out: HashMap<u64, Vec<usize>, RandomState> = HashMap::new();
    for (i, e) in entries.iter().enumerate() {
        let size = match e.stat_size {
            None => continue,
            Some(x) => x,
        };
        out.entry(size)
            .or_default()
            .push(i)
    }
    out
}


pub struct FilesIndex<'a> {
    entries: &'a [FileEntry<'a>],
    by_path: HashMap<PathBuf, usize>,
    by_size: HashMap<u64, Vec<usize>>,
}

impl<'a> FilesIndex<'a> {
    fn from_entries(entries: &'a [FileEntry]) -> Self {
        let by_path = entries.iter()
            .enumerate()
            .map(|(i, e)| (e.relative_path.to_owned(), i))
            .collect();

        Self {
            entries,
            by_path,
            by_size: group_by_size(entries),
        }
    }


    pub fn get_by_path<P: AsRef<Path>>(&self, path: &P) -> Option<&'a FileEntry> {
        self.by_path.get(path.as_ref())
            .map(|&i| { self.entries[i].borrow() })
    }
}

#[cfg(test)]
mod test {
    use std::path::{PathBuf, Path};
    use crate::lib::file_entry::FileEntry;
    use crate::lib::fs::TestFs;
    use crate::lib::files_index::FilesIndex;

    #[test]
    pub fn test_construct() {
        let mut test_fs = TestFs::default();
        test_fs.add_text_file("/somefolder/filepath", "test");
        let base_path = Path::new("/somefolder/");
        test_fs.set_cwd(base_path);

        let make_file_entry = |path: &str| -> FileEntry {
            FileEntry::new(&test_fs, base_path, Path::new(path)).unwrap()
        };

        let file_entries = vec![
            make_file_entry("asdf"),
            make_file_entry("asdf2"),
            make_file_entry("newfile"),
        ];

        let index = FilesIndex::from_entries(&file_entries);
        assert_eq!(index.entries.len(), 3);
        assert_eq!(index.by_path.len(), 3);
        // nothing here has stat
        assert_eq!(index.by_size.len(), 0);

        assert_eq!(index.get_by_path(&"asdf").unwrap(), &file_entries[0]);
        assert_eq!(index.get_by_path(&"asdf2").unwrap(), &file_entries[1]);
        assert_eq!(index.get_by_path(&"newfile").unwrap(), &file_entries[2]);

    }
}


