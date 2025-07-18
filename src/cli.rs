use std::path::PathBuf;

use clap::{Parser, ValueEnum};

#[derive(Parser)]
#[command(
    name = "audible-util",
    about = "A utility for converting Audible .aaxc files to common audio formats (mp3, wav, flac), with optional splitting by chapters.\n\n\
USAGE EXAMPLES:\n\
  audible-util -a mybook.aaxc --voucher_path my.voucher --output_type mp3 --split\n\
  audible-util -a mybook.aaxc --output_path output.wav\n\n\
TIPS:\n\
  - You must provide a valid .aaxc file as input.\n\
  - A voucher file is required for decryption. See --voucher_path for details.\n\
  - Ensure ffmpeg is installed and available in your PATH.\n\
  - Splitting is only supported for formats that support chapters (e.g., mp3, flac).\n\
  - Output file type defaults to wav if not specified.\n"
)]
pub struct Cli {
    /// Path to the input .aaxc file to convert.
    ///
    /// Example: -a mybook.aaxc
    #[clap(short = 'a', long = "aaxc_path", value_name = "AAXC_FILE", help = "Path to the input .aaxc file to convert. This file must be downloaded from Audible. Example: -a mybook.aaxc")]
    pub aaxc_path: PathBuf,

    /// Path to the voucher file required for decryption.
    ///
    /// The voucher file is needed to decrypt the .aaxc file. You can obtain it using the Audible app or other tools.
    /// Example: --voucher_path my.voucher
    /// TIP: The voucher file must match the account used to download the .aaxc file.
    #[clap(long, value_name = "VOUCHER_FILE", help = "Path to the voucher file required for decryption. Example: --voucher_path my.voucher. TIP: The voucher file must match the account used to download the .aaxc file.")]
    pub voucher_path: Option<PathBuf>,

        /// Path to the output audio file or directory.
        ///
        /// If a file path is provided, it will be used as the output file.
        /// If a directory is provided, the output file will be created inside that directory using the default naming scheme (e.g., <album>.<ext>).
        /// If not specified, the output file will be created in the current directory with the same base name as the input.
        /// Example: --output_path output.mp3 or --output_path /path/to/output_dir
        #[clap(
            long,
            value_name = "OUTPUT_PATH",
            help = "Path to the output audio file or directory. If a file path is provided, it will be used as the output file. If a directory is provided, the output file will be created inside that directory using the default naming scheme (e.g., <album>.<ext>). If not specified, the output will be placed in the current directory."
        )]
        pub output_path: Option<PathBuf>,

    /// Split the output audio file by chapters.
    ///
    /// If set, the output will be split into separate files for each chapter (if chapter information is available).
    /// NOTE: Splitting is only supported for formats that support chapters (e.g., mp3, flac).
    #[clap(short, long, help = "Split the output audio file by chapters. Only supported for formats that support chapters (e.g., mp3, flac).")]
    pub split: bool,

    /// Output file type/format.
    ///
    /// Supported values: mp3, wav, flac
    /// Example: --output_type mp3
    /// If not specified, defaults to wav.
    #[clap(long, value_enum, value_name = "TYPE", help = "Output file type/format. Supported: mp3, wav, flac. Example: --output_type mp3. Defaults to wav if not specified.")]
    pub output_type: Option<OutputType>,
}

pub trait OutputFormat {
    fn codec(&self) -> &'static str;
    fn extension(&self) -> &'static str;
}

pub struct Mp3Format;
pub struct WavFormat;
pub struct FlacFormat;

impl OutputFormat for Mp3Format {
    fn codec(&self) -> &'static str { "mp3" }
    fn extension(&self) -> &'static str { "mp3" }
}
impl OutputFormat for WavFormat {
    fn codec(&self) -> &'static str { "pcm_s16le" }
    fn extension(&self) -> &'static str { "wav" }
}
impl OutputFormat for FlacFormat {
    fn codec(&self) -> &'static str { "flac" }
    fn extension(&self) -> &'static str { "flac" }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum OutputType {
    /// MPEG Layer 3 Audio (.mp3)
    Mp3,
    /// Waveform Audio File Format (.wav)
    Wav,
    /// Free Lossless Audio Codec (.flac)
    Flac,
}

impl OutputType {
    pub fn get_format(&self) -> Box<dyn OutputFormat> {
        match self {
            OutputType::Mp3 => Box::new(Mp3Format),
            OutputType::Wav => Box::new(WavFormat),
            OutputType::Flac => Box::new(FlacFormat),
        }
    }
}
