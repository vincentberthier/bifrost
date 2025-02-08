// File: src/io/vault.rs
// Project: Bifrost
// Creation date: Sunday 09 February 2025
// Author: Vincent Berthier <vincent.berthier@posteo.org>
// -----
// Last modified: Sunday 09 February 2025 @ 01:30:51
// Modified by: Vincent Berthier
// -----
// Copyright (c) 2025 <Vincent Berthier>
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the 'Software'), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED 'AS IS', WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

use tracing::{debug, instrument};

use super::{support::create_folder, Result};

pub static VAULT_PATH: OnceLock<PathBuf> = OnceLock::new();

#[mutants::skip]
#[expect(clippy::unwrap_used)]
pub fn set_vault_path(path: &str) {
    VAULT_PATH.set(Path::new(path).to_path_buf()).unwrap();
}

#[expect(clippy::expect_used)]
pub fn get_vault_path() -> &'static PathBuf {
    VAULT_PATH.get().expect("vault path is not set")
}

#[mutants::skip]
#[instrument]
pub fn init_vault() -> Result<()> {
    debug!("initializing vault");
    let path = get_vault_path();
    ["accounts", "transactions", "blocks"]
        .iter()
        .map(|&folder| path.join(folder))
        .try_for_each(create_folder)?;

    Ok(())
}
