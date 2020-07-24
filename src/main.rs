extern crate fasthash;
extern crate walkdir;

use walkdir::WalkDir;

use lib::fs::ReadOnlyFs;
use lib::Result;
use crate::lib::files_index::FilesIndex;
use crate::lib::fs::AbstractFs;
use std::path::Path;

mod lib;

fn main() {
    let base_path = "/home/j0sh/Dropbox/Pics/meme images";
    let mut files_index = FilesIndex::new(base_path);
    let fs = ReadOnlyFs {};

    WalkDir::new("/home/j0sh/Dropbox/Pics/meme images")
        .into_iter()
        .for_each(|r| {
            match r {
                Ok(f) if f.file_type().is_file() => {
                    if let Err(e) = files_index.add_file(&fs, f.path()) {
                        println!("{}: {:?}", f.path().display(), e);
                    }
                }
                // directory or symlink, we don't care
                Ok(_) => (),
                Err(_) => (),
            }
        });
    println!("{:?}", files_index);

    println!("Hello, world!");
}

