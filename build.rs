// Copyright © 2022 Daniel Getz
// SPDX-License-Identifier: MIT

use std::fs::File;
use std::io::Write;

use git2::{DescribeFormatOptions, DescribeOptions, Repository};
use shadow_rs::SdResult;

fn main() -> SdResult<()> {
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=Cargo.lock");
    shadow_rs::new_hook(shadow_hook)
}

fn shadow_hook(mut file: &File) -> SdResult<()> {
    writeln!(
        file,
        "pub const GIT_DESCRIBE: &str = {:?};",
        get_describe_version().unwrap()
    )?;
    Ok(())
}

fn get_describe_version() -> Result<String, git2::Error> {
    let repo = Repository::open(".")?;
    let description =
        repo.describe(DescribeOptions::default().show_commit_oid_as_fallback(true))?;
    description.format(Some(DescribeFormatOptions::default().dirty_suffix("*")))
}
