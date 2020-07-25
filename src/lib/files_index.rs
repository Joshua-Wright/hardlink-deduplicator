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
use crate::lib::fast_hash::hash_file;


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
    pub base_path: PathBuf,
    entries: Vec<FileEntry>,
    by_relative_path: HashMap<PathBuf, usize>,
    by_size: HashMap<u64, HashSet<usize>>,
    by_inode: HashMap<u64, HashSet<usize>>,
    by_hash: HashMap<u128, HashSet<usize>>,
    inode_by_size: HashMap<u64, HashSet<u64>>,
    inode_by_hash: HashMap<u128, HashSet<u64>>,
}

impl FilesIndex {
    pub fn new<P: AsRef<Path>>(base_path: P) -> Self {
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
                |e| e.fast_hash.unwrap(),
            ),
            inode_by_size: group_by_with_value_func(
                entries,
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

    pub fn for_base_path<Fs: AbstractFs, P: AsRef<Path>>(fs: &Fs, base_path: P) -> Result<Self> {
        let mut index_path = base_path.as_ref().to_owned();
        index_path.push(".index_file.csv");

        let file = fs.open(index_path)?;
        let mut rdr = csv::Reader::from_reader(file);

        let entries: Vec<FileEntry> = rdr.deserialize()
            .collect::<std::result::Result<_, _>>()?;

        Ok(Self::from_entries(base_path, &entries))
    }

    pub fn save<Fs: AbstractFs>(&self, fs: &mut Fs) -> Result<()> {
        let mut index_path = self.base_path.clone();
        index_path.push(".index_file.csv");

        let file = fs.open_writable(index_path)?;
        let mut wtr = csv::Writer::from_writer(file);
        for entry in &self.entries {
            wtr.serialize(entry)?;
        }
        Ok(())
    }

    pub fn sanity_check(&self) {
        // check starting from the file entries
        for (i, entry) in self.entries.iter().enumerate() {
            assert!(self.by_relative_path.get(&entry.relative_path).unwrap() == &i);
            assert!(self.by_size.get(&entry.stat_size).unwrap().contains(&i));
            assert!(self.by_inode.get(&entry.stat_inode).unwrap().contains(&i));
            assert!(self.inode_by_size.get(&entry.stat_size).unwrap().contains(&entry.stat_inode));
            if let Some(hash) = entry.fast_hash {
                assert!(self.by_hash.get(&hash).unwrap().contains(&i));
                assert!(self.inode_by_hash.get(&hash).unwrap().contains(&entry.stat_inode));
            }
        }

        // and now check starting from the indexes
        self.by_size.iter()
            .flat_map(|(key, idxs)|
                idxs
                    .iter()
                    .map(move |&idx| (key, idx))
            )
            .map(|(key, idx)| (key, &self.entries[idx]))
            .for_each(|(key, entry)| {
                assert_eq!(key, &entry.stat_size);
            });
        self.by_inode.iter()
            .flat_map(|(key, idxs)|
                idxs
                    .iter()
                    .map(move |&idx| (key, idx))
            )
            .map(|(key, idx)| (key, &self.entries[idx]))
            .for_each(|(key, entry)| {
                assert_eq!(key, &entry.stat_inode);
            });
        self.by_hash.iter()
            .flat_map(|(key, idxs)|
                idxs
                    .iter()
                    .map(move |&idx| (key, idx))
            )
            .map(|(key, idx)| (key, &self.entries[idx]))
            .for_each(|(key, entry)| {
                assert_eq!(key, &entry.fast_hash.unwrap());
            });

        // and check that the hashes are consistent, just to make sure
        for (_, idxs) in self.by_inode.iter() {
            let hashes: HashSet<_> = idxs.iter()
                .map(|&i| &self.entries[i].fast_hash)
                .collect();
            assert_eq!(hashes.len(), 1);
        }

        // check that things are hashed when they should be
        self.by_size.iter()
            .filter(|(_, idxs)| idxs.len() > 1)
            .flat_map(|(_, idxs)|
                idxs
                    .iter()
                    .map(move |&idx| &self.entries[idx])
            )
            .for_each(|entry| {
                assert!(&entry.fast_hash.is_some(),
                        "file relative_path={:?} is missing hash (size {} is non-unique)",
                        entry.relative_path.to_string_lossy(),
                        entry.stat_size,
                );
            });
    }


    pub fn get_by_relative_path<P: AsRef<Path>>(&self, relative_path: &P) -> Option<&FileEntry> {
        self.by_relative_path.get(relative_path.as_ref())
            .map(|&i| { &self.entries[i] })
    }

    pub fn update_file_entry(&mut self, file_entry: &FileEntry) -> &FileEntry {
        let idx = if let Some(&idx) = self.by_relative_path.get(&file_entry.relative_path) {
            let existing_entry = &self.entries[idx];
            // if this wouldn't update anything useful, just short-circuit
            if file_entry.eq_except_hash(&existing_entry) && file_entry.fast_hash.is_none() {
                return &self.entries[idx];
            }
            // new replacement for existing index, remove existing stuff
            self.by_size.get_mut(&existing_entry.stat_size).unwrap().remove(&idx);
            self.by_inode.get_mut(&existing_entry.stat_inode).unwrap().remove(&idx);
            self.inode_by_size.get_mut(&existing_entry.stat_size).unwrap().remove(&existing_entry.stat_inode);
            if let Some(hash) = existing_entry.fast_hash {
                self.by_hash.get_mut(&hash).unwrap().remove(&idx);
                self.inode_by_hash.get_mut(&hash).unwrap().remove(&existing_entry.stat_inode);
            }
            // and re-insert, because the entry has probably changed
            self.entries[idx] = file_entry.clone();
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

    // TODO: get this to work
    // pub fn add_file_if_trivial_unique(&mut self, new_entry: &FileEntry) -> Option<&FileEntry> {
    //     // this file is already deduplicated into this index
    //     if let Some(existing) = self.by_inode.get(&new_entry.stat_inode) {
    //         // grab the hash, if possible
    //         let &idx = existing.iter().next().unwrap();
    //         let fast_hash = self.entries[idx].fast_hash;
    //         return self.insert_or_overwrite_file_entry(
    //             &FileEntry {
    //                 fast_hash: fast_hash,
    //                 ..new_entry.clone()
    //             }
    //         ).into();
    //     }
    //
    //     if !self.by_size.contains_key(&new_entry.stat_size) {
    //         // this file is unique because nothing in the index could match this file by length
    //         return self.insert_or_overwrite_file_entry(&new_entry).into();
    //     }
    //     None
    // }

    fn hard_link_and_insert<Fs: AbstractFs>(&mut self, fs: &mut Fs,
                                            existing_entry: &FileEntry,
                                            new_entry: &FileEntry,
    ) -> Result<&FileEntry> {
        // TODO: implement dry-run mode
        assert_eq!(new_entry.stat_size, existing_entry.stat_size);
        assert_eq!(new_entry.fast_hash, existing_entry.fast_hash);
        assert_ne!(new_entry.stat_inode, existing_entry.stat_inode);

        let mut backup_filename = new_entry.relative_path.file_name().unwrap().to_owned();
        backup_filename.push(".backup");
        let backup_abs_path = self.base_path.join(new_entry.relative_path.with_file_name(backup_filename));
        fs.rename(new_entry.absolute_path(&self.base_path), &backup_abs_path)?;
        fs.hard_link(existing_entry.absolute_path(&self.base_path), new_entry.absolute_path(&self.base_path))?;
        // TODO: rename the file back, if hard link fails

        let mut checked_new_entry = FileEntry::new(fs, &self.base_path, &new_entry.relative_path)?;
        checked_new_entry.fast_hash = new_entry.fast_hash;
        assert_eq!(
            (checked_new_entry.fast_hash, checked_new_entry.stat_size, checked_new_entry.stat_inode),
            (existing_entry.fast_hash, existing_entry.stat_size, existing_entry.stat_inode),
            "fatal error linking {} to {}",
            existing_entry.relative_path.to_string_lossy(), new_entry.relative_path.to_string_lossy());

        fs.remove_file(&backup_abs_path)?;
        Ok(self.update_file_entry(&checked_new_entry))
    }

    pub fn add_file<Fs: AbstractFs, P: AsRef<Path>>(&mut self, fs: &mut Fs, path: P) -> Result<&FileEntry> {
        let mut new_entry = FileEntry::new(fs, &self.base_path, path)?;

        if self.by_inode.contains_key(&new_entry.stat_inode) {
            // this file is already deduplicated into this index
            return Ok(self.update_file_entry(&new_entry));
        }

        if !self.by_size.contains_key(&new_entry.stat_size) {
            // this file is unique because nothing in the index could match this file by length
            return Ok(self.update_file_entry(&new_entry));
        }

        // safe to unwrap because we checked the key is there above
        let potential_dupes = self.by_size.get(&new_entry.stat_size).unwrap();

        if potential_dupes.len() == 1 {
            let &existing_entry_idx = potential_dupes.iter().next().unwrap();
            let existing_entry = self.entries[existing_entry_idx].clone();
            match self.compare_files(fs, &existing_entry, &new_entry, false)? {
                (equal, Some((existing_entry_hash, new_entry_hash))) => {
                    let updated_existing_entry = FileEntry {
                        fast_hash: Some(existing_entry_hash),
                        ..existing_entry
                    };
                    self.update_file_entry(&updated_existing_entry);
                    new_entry.fast_hash = Some(new_entry_hash);
                    if equal {
                        // they are equal, so this is a duplicate file
                        return Ok(self.hard_link_and_insert(fs, &updated_existing_entry, &new_entry)?);
                    } else {
                        // it's a non-duplicate, so just insert it
                        return Ok(self.update_file_entry(&new_entry));
                    }
                }
                // we told self.compare_files() not to short-circuit, so it better not have short-circuited
                (_, None) => { unreachable!(); }
            }
        }

        // file is non-unique in length, so we will now hash the whole thing
        new_entry.fast_hash = Some(hash_file(fs, &new_entry.absolute_path(&self.base_path))?);

        // now compare by hash to insert
        let potential_dupes = match self.by_size.get(&new_entry.stat_size) {
            None => {
                // no duplicates, so we will just insert
                return Ok(self.update_file_entry(&new_entry));
            }
            Some(x) => x,
        };

        // from now on, we don't need to hash anything (so we can always short-circuit when we insert
        for &idx in potential_dupes {
            // must clone this so we don't borrow self
            let existing_entry = self.entries[idx].clone();
            match self.compare_files(fs, &existing_entry, &new_entry, true)? {
                (false, _) => continue,
                (true, _) =>
                // match found! we can now short-circuit
                    return Ok(self.hard_link_and_insert(fs, &existing_entry, &new_entry)?),
            }
        }

        // if we get this far, then that means we didn't find any matches, and this file is unique
        return Ok(self.update_file_entry(&new_entry));
    }


    fn compare_files<Fs: AbstractFs>(&self, fs: &Fs, entry1: &FileEntry, entry2: &FileEntry, short_circuit: bool) -> Result<(bool, Option<(u128, u128)>)> {
        const BUFSIZE: usize = 4096;
        let file1 = fs.open(&entry1.absolute_path(&self.base_path))?;
        let file2 = fs.open(&entry2.absolute_path(&self.base_path))?;
        let mut reader1 = BufReader::with_capacity(BUFSIZE, file1);
        let mut reader2 = BufReader::with_capacity(BUFSIZE, file2);

        let mut hasher1: murmur3::Hasher128_x64 = Default::default();
        let mut hasher2: murmur3::Hasher128_x64 = Default::default();

        let mut equal = true;
        loop {
            let (len1, len2) = match (reader1.fill_buf(), reader2.fill_buf()) {
                (Ok(buf1), Ok(buf2)) => {
                    if buf1 != buf2 {
                        equal = false;
                        if short_circuit {
                            return Ok((false, None));
                        }
                    }
                    if buf1.is_empty() {
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

        Ok((equal, (hasher1.finish_ext().into(), hasher2.finish_ext().into()).into()))
    }
}


#[cfg(test)]
mod test {
    use std::path::Path;

    use crate::lib::files_index::FilesIndex;
    use crate::lib::fs::TestFs;
    use std::collections::HashSet;

    #[test]
    pub fn test_construct() {
        let mut test_fs = TestFs::default();
        test_fs.add_text_file("/somefolder/filepath", "test");
        let base_path = Path::new("/somefolder/");
        test_fs.set_cwd(base_path);

        let file_entries = vec![
            test_fs.new_file_entry("/somefolder/asdf", "test"),
            test_fs.new_file_entry("/somefolder/asdf2", "asdf1"),
            test_fs.new_file_entry("/somefolder/newfile", "newfile"),
        ];

        let index = FilesIndex::from_entries("/somefolder/", &file_entries);
        assert_eq!(index.entries.len(), 3);
        assert_eq!(index.by_relative_path.len(), 3);
        assert_eq!(index.by_size.len(), 3);

        assert_eq!(index.get_by_relative_path(&"asdf").unwrap(), &file_entries[0]);
        assert_eq!(index.get_by_relative_path(&"asdf2").unwrap(), &file_entries[1]);
        assert_eq!(index.get_by_relative_path(&"newfile").unwrap(), &file_entries[2]);

        index.sanity_check();
    }


    #[test]
    pub fn test_add_unique_file() {
        let mut test_fs = TestFs::default();
        let base_path = Path::new("/somefolder/");
        test_fs.set_cwd(base_path);

        let mut index = FilesIndex::new(base_path);

        let f1 = test_fs.new_file_entry("/somefolder/asdf", "test");
        index.add_file(&mut test_fs, f1.relative_path.as_path()).unwrap();
        assert_eq!(index.get_by_relative_path(&f1.relative_path.as_path()).unwrap(), &f1);
        assert_eq!(index.entries.len(), 1);
        assert_eq!(index.by_relative_path.len(), 1);
        assert_eq!(index.by_size.len(), 1);


        let f2 = test_fs.new_file_entry("/somefolder/asdfasdf", "testasdf");
        index.add_file(&mut test_fs, f2.relative_path.as_path()).unwrap();
        assert_eq!(index.get_by_relative_path(&f2.relative_path.as_path()).unwrap(), &f2);
        assert_eq!(index.entries.len(), 2);
        assert_eq!(index.by_relative_path.len(), 2);
        assert_eq!(index.by_size.len(), 2);

        // test with adding two un-equal files with the same size
        let f1 = test_fs.new_file_entry("/somefolder/test1", "test1 asdf asdf");
        let f2 = test_fs.new_file_entry("/somefolder/test2", "test2 asdf asdf");
        index.add_file(&mut test_fs, f1.relative_path.as_path()).unwrap();
        index.add_file(&mut test_fs, f2.relative_path.as_path()).unwrap();
        assert_eq!(index.by_size.len(), 3);
        assert_eq!(index.by_size.get(&15).unwrap().len(), 2);

        index.sanity_check();
    }

    #[test]
    pub fn test_one_duplicate() {
        let mut test_fs = TestFs::default();
        let base_path = Path::new("/somefolder/");
        test_fs.set_cwd(base_path);

        let mut index = FilesIndex::new(base_path);

        let f1 = test_fs.new_file_entry("/somefolder/test1", "asdf");
        let f2 = test_fs.new_file_entry("/somefolder/test2", "asdf");
        assert_ne!(f1.stat_inode, f2.stat_inode);
        index.add_file(&mut test_fs, f1.relative_path.as_path()).unwrap();
        index.add_file(&mut test_fs, f2.relative_path.as_path()).unwrap();
        let f1 = index.get_by_relative_path(&f1.relative_path).unwrap();
        let f2 = index.get_by_relative_path(&f2.relative_path).unwrap();
        index.sanity_check();
        assert!(f1.fast_hash.is_some());
        assert!(f2.fast_hash.is_some());
        assert_eq!(f1.stat_inode, f2.stat_inode);
        assert_eq!(f1.fast_hash, f2.fast_hash);
    }

    #[test]
    pub fn test_two_duplicates() {
        let mut test_fs = TestFs::default();
        let base_path = Path::new("/somefolder/");
        test_fs.set_cwd(base_path);

        let mut index = FilesIndex::new(base_path);

        let f1 = test_fs.new_file_entry("/somefolder/test1", "asdf");
        let f2 = test_fs.new_file_entry("/somefolder/test2", "asdf");
        let f3 = test_fs.new_file_entry("/somefolder/test3", "asdf");
        assert_ne!(f1.stat_inode, f2.stat_inode);
        assert_ne!(f1.stat_inode, f3.stat_inode);
        assert_ne!(f2.stat_inode, f3.stat_inode);
        index.add_file(&mut test_fs, f1.relative_path.as_path()).unwrap();
        index.add_file(&mut test_fs, f2.relative_path.as_path()).unwrap();
        index.add_file(&mut test_fs, f3.relative_path.as_path()).unwrap();
        let f1 = index.get_by_relative_path(&f1.relative_path).unwrap();
        let f2 = index.get_by_relative_path(&f2.relative_path).unwrap();
        let f3 = index.get_by_relative_path(&f3.relative_path).unwrap();
        index.sanity_check();
        assert!(f1.fast_hash.is_some());
        assert!(f2.fast_hash.is_some());
        assert!(f3.fast_hash.is_some());
        assert_eq!(f1.stat_inode, f2.stat_inode);
        assert_eq!(f1.fast_hash, f2.fast_hash);
        assert_eq!(f1.fast_hash, f3.fast_hash);
    }

    #[test]
    pub fn test_write_to_csv() {
        let mut test_fs = TestFs::default();
        let base_path = Path::new("/somefolder/");
        test_fs.set_cwd(base_path);

        let mut index = FilesIndex::new(base_path);

        let f1 = test_fs.new_file_entry("/somefolder/test1", "asdf");
        let f2 = test_fs.new_file_entry("/somefolder/test2", "asdf");
        let f3 = test_fs.new_file_entry("/somefolder/test3", "asdf");
        index.add_file(&mut test_fs, f1.relative_path.as_path()).unwrap();
        index.add_file(&mut test_fs, f2.relative_path.as_path()).unwrap();
        index.add_file(&mut test_fs, f3.relative_path.as_path()).unwrap();
        index.sanity_check();

        index.save(&mut test_fs).unwrap();
        let s = test_fs.get_file_data("/somefolder/.index_file.csv").unwrap();
        let s = std::str::from_utf8(s).unwrap();
        assert_eq!(s, "relative_path,fast_hash,stat_size,stat_modified,stat_accessed,stat_created,stat_inode
test1,290827534275623791776536726795751555336,4,1970-01-01T00:00:00Z,1970-01-01T00:00:00Z,1970-01-01T00:00:00Z,2
test2,290827534275623791776536726795751555336,4,1970-01-01T00:00:00Z,1970-01-01T00:00:00Z,1970-01-01T00:00:00Z,2
test3,290827534275623791776536726795751555336,4,1970-01-01T00:00:00Z,1970-01-01T00:00:00Z,1970-01-01T00:00:00Z,2
");
    }

    #[test]
    pub fn test_read_from_csv() {
        let mut test_fs = TestFs::default();
        let base_path = Path::new("/somefolder/");
        test_fs.set_cwd(base_path);

        let f1 = test_fs.new_file_entry("/somefolder/test1", "asdf");
        let f2 = test_fs.new_file_entry("/somefolder/test2", "asdf");
        let f3 = test_fs.new_file_entry("/somefolder/test3", "asdf");
        test_fs.add_text_file("/somefolder/.index_file.csv",
                                               "relative_path,fast_hash,stat_size,stat_modified,stat_accessed,stat_created,stat_inode
test1,290827534275623791776536726795751555336,4,1970-01-01T00:00:00Z,1970-01-01T00:00:00Z,1970-01-01T00:00:00Z,2
test2,290827534275623791776536726795751555336,4,1970-01-01T00:00:00Z,1970-01-01T00:00:00Z,1970-01-01T00:00:00Z,2
test3,290827534275623791776536726795751555336,4,1970-01-01T00:00:00Z,1970-01-01T00:00:00Z,1970-01-01T00:00:00Z,2
");

        let mut index = FilesIndex::for_base_path(&test_fs, base_path).unwrap();
        index.sanity_check();

        let f1 = index.get_by_relative_path(&f1.relative_path).unwrap().clone();
        let f2 = index.get_by_relative_path(&f2.relative_path).unwrap().clone();
        let f3 = index.get_by_relative_path(&f3.relative_path).unwrap().clone();
        assert!(f1.fast_hash.is_some());
        assert!(f2.fast_hash.is_some());
        assert!(f3.fast_hash.is_some());
        assert_eq!(f1.stat_inode, f2.stat_inode);
        assert_eq!(f1.fast_hash, f2.fast_hash);
        assert_eq!(f1.fast_hash, f3.fast_hash);

        index.add_file(&mut test_fs, f1.relative_path.as_path()).unwrap();
        index.add_file(&mut test_fs, f2.relative_path.as_path()).unwrap();
        index.add_file(&mut test_fs, f3.relative_path.as_path()).unwrap();
        index.sanity_check();
    }

    #[test]
    pub fn test_stress_test() {
        let mut test_fs = TestFs::default();
        let base_path = Path::new("/largefolder/");
        test_fs.set_cwd(base_path);

        let mut index = FilesIndex::new(base_path);

        let mut file_content = HashSet::new();

        for i in 1..200 {
            let content = format!("file_{}", (i % 42)).repeat(i % 3);
            file_content.insert(content.clone());
            let f1 = test_fs.new_file_entry(format!("/largefolder/file_{}", i).as_str(),
                                            &content,
            );
            index.add_file(&mut test_fs, f1.relative_path.as_path()).unwrap();
        }
        index.sanity_check();
        assert_eq!(file_content.len(), index.by_inode.len());
        assert!(index.by_inode.len() < 199);
        assert!(index.by_size.len() < 199);
        assert!(index.by_hash.len() < 199);
        assert!(index.inode_by_size.len() < 199);
        assert!(index.inode_by_hash.len() < 199);
        assert_eq!(index.by_relative_path.len(), 199);
        assert_eq!(index.by_inode.len(), 29);
        // TODO: find examples of hash collisions and test that
        assert_eq!(index.by_hash.len(), 29);
    }
}


