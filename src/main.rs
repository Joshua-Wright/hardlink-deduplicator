extern crate fasthash;
extern crate walkdir;

use walkdir::WalkDir;

use lib::fs::ReadOnlyFs;
use lib::Result;
use crate::lib::files_index::FilesIndex;
use crate::lib::fs::{AbstractFs, RealFs};
use std::path::Path;

mod lib;

extern crate clap;
use clap::Clap;

#[derive(Clap, Debug)]
#[clap(version = "1.0", about = "deduplicates files")]
struct Opts {
    /// folder to deduplicate files in
    folder: String,
    /// Print test information verbosely
    #[clap(short, long)]
    verbose: bool,
    /// don't print anything, not even errors
    #[clap(short, long)]
    quiet: bool,
    /// if true, no filesystem changes will be made
    #[clap(short, long)]
    dry_run: bool,
}


fn main() {
    let opts: Opts = Opts::parse();

    println!("{:?}", opts);
    if opts.dry_run {
        println!("running a dry run");
        let mut fs = ReadOnlyFs {};
        run_for_folder(&mut fs, opts.folder).unwrap();
    } else {
        // let fs = RealFs {};
        let mut fs = ReadOnlyFs {};
        run_for_folder(&mut fs, opts.folder).unwrap();
    }

}

fn run_for_folder<Fs: AbstractFs, P: AsRef<Path>>(fs: &mut Fs, path: P) -> Result<()> {
    let base_path = std::fs::canonicalize(path)?;
    let mut files_index = FilesIndex::new(&base_path);

    WalkDir::new(&base_path)
        .into_iter()
        .for_each(|r| {
            match r {
                Ok(f) if f.file_type().is_file() => {
                    if let Err(e) = files_index.add_file(fs, f.path()) {
                        println!("{}: {:?}", f.path().display(), e);
                    }
                }
                // directory or symlink, we don't care
                Ok(_) => (),
                Err(_) => (),
            }
        });
    println!("{:?}", files_index);
    Ok(())
}
