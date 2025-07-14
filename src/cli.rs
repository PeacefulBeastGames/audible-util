use std::path::PathBuf;

use clap::{Parser, ValueEnum};

#[derive(Parser)]
pub struct Cli {
    /// Path to aaxc file
    #[clap(short, long)]
    pub aaxc_path: PathBuf,

    /// voucher file
    #[clap(long)]
    pub voucher_path: Option<PathBuf>,
    // Ideal interface
    // I need to get a path to the audio file which can be either aaxc or aax
    // Optionally I can get the output path
    // Also I need to a flag to determine whether to split the final file or not
    // I might want to add an option to choose the output file type like mp3, flac etc...
    #[clap(long)]
    pub output_path: Option<PathBuf>,

    #[clap(short, long)]
    pub split: bool,

    /// Output file type enum
    #[clap(long, value_enum)]
    pub output_type: Option<OutputType>,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum OutputType {
    Mp3,
    Wav,
    Flac,
}
