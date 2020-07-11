extern crate walkdir;
extern crate fasthash;

use walkdir::WalkDir;
use walkdir::DirEntry;
use walkdir::Error;
use std::any::Any;

use fasthash::murmur3;
use fasthash::HasherExt;
use fasthash::StreamHasher;


use std::hash::Hash;
use std::path::Path;
use std::io::Read;
use std::ops::DerefMut;

// this is not very good beacuse it confuses intellij
#[cfg(not(test))]
use std::fs::File;

#[cfg(test)]
struct File {
}

#[cfg(test)]
impl File {
    pub fn open<P: AsRef<Path>>(path: P) -> std::io::Result<std::io::Cursor<&'static[u8]>> {
        let s: &'static str = "test";
        Ok(std::io::Cursor::new(s.as_bytes()))
    }
}


fn hash_file(path: &Path) -> std::io::Result<u128> {
    let mut file = File::open(path)?;
    let mut hasher: murmur3::Hasher128_x64 = Default::default();
    StreamHasher::write_stream(&mut hasher, &mut file)?;
    // hasher.write_stream(&mut file)?;
    Ok(hasher.finish_ext())
}


fn main() {
    WalkDir::new("/home/j0sh/Dropbox/Pics/meme images")
        .into_iter()
        .for_each(|r| {
            match r {
                Ok(f) if f.file_type().is_file() =>
                    match hash_file(f.path()) {
                        Ok(hash) =>
                            println!("{:032X} {}", hash, f.path().to_string_lossy()),
                        Err(e) =>
                        // (),
                            println!("{:?}: Error: {:?}", f.path().to_string_lossy(), e),
                    },
                // directory or symlink, we don't care
                Ok(_) => (),
                Err(_) => (),
            }
        });

    println!("Hello, world!");
}


#[test]
fn test_mock_file() {
    let mut f = File::open("asdf").unwrap();
    let mut s = String::new();
    f.read_to_string(&mut s).unwrap();
    println!("{}", s);
    assert_eq!(s, "test");
}


#[test]
fn test_hash_file() {
    let hash = hash_file(Path::new("unused")).unwrap();
    assert_eq!(hash, 204797213367049729698754624420042367389u128);
}

