use std::path::PathBuf;

use clap::Parser;

#[derive(Parser)]
pub struct Cli {
    /// Path to aaxc file
    #[clap(short, long)]
    pub aaxc_path: PathBuf,

    /// voucher file
    #[clap(long)]
    pub voucher_path: Option<PathBuf>,
}
