//! Write the sample documents to a directory, one `.bcfzip` each.
//!
//! ```bash
//! cargo run --example write-samples -- <out-dir>
//! ```
//!
//! `scripts/validate-written.py` runs this and validates every entry of every
//! archive against the official buildingSMART XSDs.

mod fixture;

use std::path::PathBuf;

fn main() {
    let Some(dir) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("usage: write-samples <out-dir>");
        std::process::exit(2);
    };
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!("cannot create {}: {e}", dir.display());
        std::process::exit(1);
    }
    for (stem, doc) in fixture::samples() {
        let path = dir.join(format!("{stem}.bcfzip"));
        if let Err(e) = openbim_bcf::write::to_path(&doc, &path) {
            eprintln!("{}: {e}", path.display());
            std::process::exit(1);
        }
        println!("{}", path.display());
    }
}
