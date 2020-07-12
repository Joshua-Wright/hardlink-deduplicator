extern crate fasthash;
extern crate walkdir;

use walkdir::WalkDir;

use lib::fast_hash::*;
use lib::fs::RealFs;

mod lib;

fn main() {
    WalkDir::new("/home/j0sh/Dropbox/Pics/meme images")
        .into_iter()
        .for_each(|r| {
            match r {
                Ok(f) if f.file_type().is_file() =>
                    match hash_file(&RealFs::default(), f.path()) {
                        Ok(hash) =>
                            println!("{} {}", hash_to_hex_str(hash), f.path().to_string_lossy()),
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

