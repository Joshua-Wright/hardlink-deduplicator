extern crate walkdir;
extern crate fasthash;

use walkdir::WalkDir;
use walkdir::DirEntry;
use walkdir::Error;
use std::any::Any;

use fasthash::murmur3;
use fasthash::HasherExt;
use fasthash::StreamHasher;


use std::fs::File;
use std::hash::Hash;
use std::path::Path;
use std::io::Read;
use std::ops::DerefMut;


fn hash_file(path: &Path) -> std::io::Result<u128> {
    let mut file = File::open(path)?;
    let mut hasher: murmur3::Hasher128_x64 = Default::default();
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;
    buffer.hash(&mut hasher);
    // TODO: change the crate so that this works
    // StreamHasher::write_stream(&mut hasher, &mut file)?;
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
