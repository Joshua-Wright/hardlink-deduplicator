use std::collections::hash_map::RandomState;
use std::collections::{HashMap, HashSet};
use std::option::Option;
use std::path::{Path, PathBuf};
use std::io::BufReader;
use std::io::BufRead;

use super::file_entry::FileEntry;
use crate::lib::fs::AbstractFs;
use crate::lib::Result;
use std::hash::{Hash, Hasher};
use fasthash::{murmur3, HasherExt};


// TODO delete this
// #[derive(Ord, PartialOrd, Eq, PartialEq, Hash, Debug)]
// enum FileEntryKey {
//     SizeOnly { size: u64 },
//     FullKey { size: u64, fast_hash: u128, unique_id: u64 },
// }


fn group_by_with_value_func<C, KF, VF, K, V>(entries: C, key_func: KF, value_func: VF) -> HashMap<K, HashSet<V>>
    where C: IntoIterator, KF: Fn(&C::Item) -> K, VF: Fn(usize, &C::Item) -> V, K: Hash + Eq, V: Hash + Eq
{
    let mut out: HashMap<K, HashSet<V>, RandomState> = HashMap::new();
    for (i, e) in entries.into_iter().enumerate() {
        out.entry(key_func(&e))
            .or_default()
            .insert(value_func(i, &e));
    }
    out
}

fn group_by<C: IntoIterator, KF, K: Hash + Eq>(entries: C, f: KF) -> HashMap<K, HashSet<usize>>
    where KF: Fn(&C::Item) -> K {
    group_by_with_value_func(entries, f, |i, _| i)
}


// invariant: all files in the files index are already deduplicated: they are either unique or they
// have the same inode
#[derive(Debug, Clone)]
pub struct FilesIndex {
    base_path: PathBuf,
    entries: Vec<FileEntry>,
    by_relative_path: HashMap<PathBuf, usize>,
    by_size: HashMap<u64, HashSet<usize>>,
    by_inode: HashMap<u64, HashSet<usize>>,
    by_hash: HashMap<u128, HashSet<usize>>,
    inode_by_size: HashMap<u64, HashSet<u64>>,
    inode_by_hash: HashMap<u128, HashSet<u64>>,
}

impl FilesIndex {
    fn new<P: AsRef<Path>>(base_path: P) -> Self {
        // TODO: look for index file and read it, I guess
        Self {
            base_path: base_path.as_ref().to_path_buf(),
            entries: Default::default(),
            by_relative_path: Default::default(),
            by_size: Default::default(),
            by_inode: Default::default(),
            by_hash: Default::default(),
            inode_by_size: Default::default(),
            inode_by_hash: Default::default(),
        }
    }

    fn from_entries<P: AsRef<Path>>(base_path: P, entries: &[FileEntry]) -> Self {
        let by_path = entries.iter()
            .enumerate()
            .map(|(i, e)| (e.relative_path.to_owned(), i))
            .collect();

        Self {
            base_path: base_path.as_ref().to_path_buf(),
            entries: entries.to_vec(),
            by_relative_path: by_path,
            by_size: group_by(entries, |e| e.stat_size),
            by_inode: group_by(entries, |e| e.stat_inode),
            by_hash: group_by(
                entries.iter().filter(|e| e.fast_hash.is_some()),
                |e| e.fast_hash.unwrap()
            ),
            inode_by_size: group_by_with_value_func(
                entries.iter().filter(|e| e.fast_hash.is_some()),
                |e| e.stat_size,
                |_, e| e.stat_inode,
            ),
            inode_by_hash: group_by_with_value_func(
                entries.iter().filter(|e| e.fast_hash.is_some()),
                |e| e.fast_hash.unwrap(),
                |_, e| e.stat_inode,
            ),
        }
    }


    pub fn get_by_relative_path<P: AsRef<Path>>(&self, relative_path: &P) -> Option<&FileEntry> {
        self.by_relative_path.get(relative_path.as_ref())
            .map(|&i| { &self.entries[i] })
    }

    pub fn insert_or_overwrite_file_entry(&mut self, file_entry: &FileEntry) -> &FileEntry {
        let idx = if let Some(&idx) = self.by_relative_path.get(&file_entry.relative_path) {
            // existing index, remove existing stuff
            self.by_size.get_mut(&self.entries[idx].stat_size).map(|s| s.remove(&idx));
            self.by_inode.get_mut(&self.entries[idx].stat_size).map(|s| s.remove(&idx));
            idx
        } else {
            // new index
            self.entries.push(file_entry.clone());
            self.entries.len() - 1
        };
        // add to indexes
        self.by_relative_path.insert(file_entry.relative_path.to_owned(), idx);
        self.by_size.entry(file_entry.stat_size).or_default().insert(idx);
        self.by_inode.entry(file_entry.stat_inode).or_default().insert(idx);
        self.inode_by_size.entry(file_entry.stat_size).or_default().insert(file_entry.stat_inode);
        if let Some(hash) = file_entry.fast_hash {
            self.by_hash.entry(hash).or_default().insert(idx);
            self.inode_by_hash.entry(hash).or_default().insert(file_entry.stat_inode);
        }
        &self.entries[idx]
    }

    pub fn add_file<Fs: AbstractFs, P: AsRef<Path>>(&mut self, fs: &Fs, relative_path: P) -> Result<&FileEntry> {
        let new_entry = FileEntry::new(fs, &self.base_path, relative_path)?;

        if self.by_inode.contains_key(&new_entry.stat_inode) {
            // this file is already deduplicated into this index
            return Ok(self.insert_or_overwrite_file_entry(&new_entry));
        }

        if !self.by_size.contains_key(&new_entry.stat_size) {
            // this file is unique because nothing in the index could match this file by length
            return Ok(self.insert_or_overwrite_file_entry(&new_entry));
        }

        // safe to unwrap because we checked the key is there above
        let potential_dupes = self.by_size.get(&new_entry.stat_size).unwrap();

        for &idx in potential_dupes {
            let existing_entry = &self.entries[idx];
            match self.compare_files(fs, existing_entry, &new_entry)? {
                (false, _) => {
                    continue;
                }
                (true, Some((e1, e2))) => {
                    // match found!
                    if e1.stat_inode == e2.stat_inode {
                        // happy case, the files are already deduplicated
                        self.insert_or_overwrite_file_entry(&e1);
                        return Ok(self.insert_or_overwrite_file_entry(&e2));
                    } else {
                        // need to actually make the hard links and deduplicate this file
                        // TODO implement deduplication
                        unimplemented!()
                    }
                }
                _ => { unreachable!() }
            }
        }
        // if we get this far, then that means we didn't find any matches, and this file is unique
        return Ok(self.insert_or_overwrite_file_entry(&new_entry));
    }


    fn compare_files<Fs: AbstractFs>(&self, fs: &Fs, entry1: &FileEntry, entry2: &FileEntry) -> Result<(bool, Option<(FileEntry, FileEntry)>)> {
        const BUFSIZE: usize = 4096;
        let file1 = fs.open(&entry1.absolute_path(&self.base_path))?;
        let file2 = fs.open(&entry2.absolute_path(&self.base_path))?;
        let mut reader1 = BufReader::with_capacity(BUFSIZE, file1);
        let mut reader2 = BufReader::with_capacity(BUFSIZE, file2);

        let mut hasher1: murmur3::Hasher128_x64 = Default::default();
        let mut hasher2: murmur3::Hasher128_x64 = Default::default();

        loop {
            let (len1, len2) = match (reader1.fill_buf(), reader2.fill_buf()) {
                (Ok(buf1), Ok(buf2)) => {
                    if buf1 != buf2 {
                        return Ok((false, None));
                    }
                    if buf1.len() == 0 {
                        break;
                    }
                    hasher1.write(buf1);
                    hasher2.write(buf2);
                    (buf1.len(), buf2.len())
                }
                (Err(e), _) => return Err(e.into()),
                (_, Err(e)) => return Err(e.into()),
            };
            reader1.consume(len1);
            reader2.consume(len2);
        }

        Ok((
            true,
            (
                FileEntry {
                    fast_hash: hasher1.finish_ext().into(),
                    ..entry1.to_owned()
                },
                FileEntry {
                    fast_hash: hasher2.finish_ext().into(),
                    ..entry2.to_owned()
                }
            ).into()
        ))
    }
}


#[cfg(test)]
mod test {
    use std::path::Path;

    use crate::lib::files_index::FilesIndex;
    use crate::lib::fs::TestFs;

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

        let index = FilesIndex::from_entries("/somefolder/", &file_entries);
        assert_eq!(index.entries.len(), 3);
        assert_eq!(index.by_relative_path.len(), 3);
        assert_eq!(index.by_size.len(), 1);

        assert_eq!(index.get_by_relative_path(&"asdf").unwrap(), &file_entries[0]);
        assert_eq!(index.get_by_relative_path(&"asdf2").unwrap(), &file_entries[1]);
        assert_eq!(index.get_by_relative_path(&"newfile").unwrap(), &file_entries[2]);
    }


    #[test]
    pub fn test_add_unique_file() {
        let mut test_fs = TestFs::default();
        let base_path = Path::new("/somefolder/");
        test_fs.set_cwd(base_path);

        let mut index = FilesIndex::new(base_path);

        let f1 = test_fs.new_file_entry("/somefolder/asdf", "test");
        index.add_file(&test_fs, f1.relative_path.as_path()).unwrap();
        assert_eq!(index.get_by_relative_path(&f1.relative_path.as_path()).unwrap(), &f1);
        assert_eq!(index.entries.len(), 1);
        assert_eq!(index.by_relative_path.len(), 1);
        assert_eq!(index.by_size.len(), 1);


        let f2 = test_fs.new_file_entry("/somefolder/asdfasdf", "testasdf");
        index.add_file(&test_fs, f2.relative_path.as_path()).unwrap();
        assert_eq!(index.get_by_relative_path(&f2.relative_path.as_path()).unwrap(), &f2);
        assert_eq!(index.entries.len(), 2);
        assert_eq!(index.by_relative_path.len(), 2);
        assert_eq!(index.by_size.len(), 2);

        // test with adding two un-equal files with the same size
        let f1 = test_fs.new_file_entry("/somefolder/test1", "test1 asdf asdf");
        let f2 = test_fs.new_file_entry("/somefolder/test2", "test2 asdf asdf");
        index.add_file(&test_fs, f1.relative_path.as_path()).unwrap();
        index.add_file(&test_fs, f2.relative_path.as_path()).unwrap();
        assert_eq!(index.by_size.len(), 3);
        assert_eq!(index.by_size.get(&15).unwrap().len(), 2);
    }

    #[test]
    #[should_panic] // TODO: implement deduplication
    pub fn test_add_duplicate_file() {
        let mut test_fs = TestFs::default();
        let base_path = Path::new("/somefolder/");
        test_fs.set_cwd(base_path);

        let mut index = FilesIndex::new(base_path);

        let f1 = test_fs.new_file_entry("/somefolder/test1", "asdf");
        let f2 = test_fs.new_file_entry("/somefolder/test2", "asdf");
        index.add_file(&test_fs, f1.relative_path.as_path()).unwrap();
        index.add_file(&test_fs, f2.relative_path.as_path()).unwrap();
    }
}


