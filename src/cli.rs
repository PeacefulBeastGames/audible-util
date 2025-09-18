use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use crate::models::ChapterNamingFormat;

#[derive(Parser)]
#[command(
    name = "audible-util",
    about,
    version,
)]
pub struct Cli {
    /// Path to the input .aaxc file to convert.
    ///
    /// Example: -a mybook.aaxc
    #[clap(short = 'a', long = "aaxc_path", value_name = "AAXC_FILE", help = "Input .aaxc file")]
    pub aaxc_path: PathBuf,

    /// Path to the voucher file required for decryption.
    ///
    /// The voucher file is needed to decrypt the .aaxc file. You can obtain it using the Audible app or other tools.
    /// Example: --voucher_path my.voucher
    /// TIP: The voucher file must match the account used to download the .aaxc file.
    #[clap(short = 'v', long, value_name = "VOUCHER_FILE", help = "Voucher file for decryption")]
    pub voucher_path: Option<PathBuf>,

        /// Path to the output audio file or directory.
        ///
        /// If a file path is provided, it will be used as the output file.
        /// If a directory is provided, the output file will be created inside that directory using the default naming scheme (e.g., <album>.<ext>).
        /// If not specified, the output file will be created in the current directory with the same base name as the input.
        /// Example: --output_path output.mp3 or --output_path /path/to/output_dir
        #[clap(
            short,
            long,
            value_name = "OUTPUT_PATH",
            help = "Output file or directory (default: current dir)"
        )]
        pub output_path: Option<PathBuf>,

    /// Split the output audio file by chapters.
    ///
    /// If set, the output will be split into separate files for each chapter (if chapter information is available).
    /// Requires a chapters.json file in the same directory as the .aaxc file.
    #[clap(short, long, help = "Split output by chapters")]
    pub split: bool,

    /// Minimum chapter duration in seconds.
    ///
    /// Chapters shorter than this duration will be skipped when splitting.
    /// Default: 0 (no minimum duration).
    #[clap(short = 'd', long, value_name = "SECONDS", help = "Minimum chapter duration in seconds")]
    pub min_chapter_duration: Option<u64>,

    /// Chapter naming format.
    ///
    /// Controls how chapter files are named when splitting.
    /// Available formats: chapter-number-title, number-title, title-only, custom
    #[clap(short = 'f', long, value_enum, value_name = "FORMAT", default_value = "chapter-number-title", help = "Chapter naming format")]
    pub chapter_naming_format: ChapterNamingFormat,

    /// Output structure for split chapters.
    ///
    /// Controls how chapter files are organized when splitting.
    /// - flat: All chapters in a single directory
    /// - hierarchical: Create folders based on chapter hierarchy
    #[clap(short = 't', long, value_enum, value_name = "STRUCTURE", default_value = "flat", help = "Output structure for split chapters")]
    pub split_structure: SplitStructure,

    /// Merge short chapters with the next chapter instead of filtering them out.
    ///
    /// When enabled, chapters shorter than --min-chapter-duration will be merged
    /// with the next chapter instead of being filtered out. This prevents gaps
    /// in the audio timeline while still allowing filtering of very short content.
    #[clap(short = 'm', long, help = "Merge short chapters with next chapter instead of filtering them out")]
    pub merge_short_chapters: bool,

    /// Output file type/format.
    ///
    /// Supported values: mp3, wav, flac, ogg, m4a
    /// Example: --output_type mp3
    #[clap(short = 'T', long, value_enum, value_name = "TYPE", default_value = "mp3", help = "Output format")]
    pub output_type: OutputType,
}

pub trait OutputFormat {
    fn codec(&self) -> &'static str;
    fn extension(&self) -> &'static str;
}

pub struct Mp3Format;
pub struct WavFormat;
pub struct FlacFormat;
pub struct AacFormat;
pub struct OggFormat;

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
impl OutputFormat for AacFormat {
    fn codec(&self) -> &'static str { "aac" }
    fn extension(&self) -> &'static str { "m4a" }
}
impl OutputFormat for OggFormat {
    fn codec(&self) -> &'static str { "vorbis" }
    fn extension(&self) -> &'static str { "ogg" }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum OutputType {
    /// MPEG Layer 3 Audio (.mp3)
    Mp3,
    /// Waveform Audio File Format (.wav)
    Wav,
    /// Free Lossless Audio Codec (.flac)
    Flac,
    /// Advanced Audio Coding (.m4a)
    M4a,
    /// Ogg Vorbis Audio (.ogg)
    Ogg
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum SplitStructure {
    /// All chapters in a single directory
    Flat,
    /// Create folders based on chapter hierarchy
    Hierarchical,
}

impl OutputType {
    pub fn get_format(&self) -> Box<dyn OutputFormat> {
        match self {
            OutputType::Mp3 => Box::new(Mp3Format),
            OutputType::Wav => Box::new(WavFormat),
            OutputType::Flac => Box::new(FlacFormat),
            OutputType::M4a => Box::new(AacFormat),
            OutputType::Ogg => Box::new(OggFormat),
        }
    }
}

impl ValueEnum for ChapterNamingFormat {
    fn value_variants<'a>() -> &'a [Self] {
        &[
            ChapterNamingFormat::ChapterNumberTitle,
            ChapterNamingFormat::NumberTitle,
            ChapterNamingFormat::TitleOnly,
        ]
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        match self {
            ChapterNamingFormat::ChapterNumberTitle => Some(clap::builder::PossibleValue::new("chapter-number-title")),
            ChapterNamingFormat::NumberTitle => Some(clap::builder::PossibleValue::new("number-title")),
            ChapterNamingFormat::TitleOnly => Some(clap::builder::PossibleValue::new("title-only")),
            ChapterNamingFormat::Custom(_) => None, // Custom formats are handled separately
        }
    }

    fn from_str(input: &str, _ignore_case: bool) -> Result<Self, String> {
        match input {
            "chapter-number-title" => Ok(ChapterNamingFormat::ChapterNumberTitle),
            "number-title" => Ok(ChapterNamingFormat::NumberTitle),
            "title-only" => Ok(ChapterNamingFormat::TitleOnly),
            custom if custom.starts_with("custom:") => {
                let pattern = custom.strip_prefix("custom:").unwrap().to_string();
                Ok(ChapterNamingFormat::Custom(pattern))
            },
            _ => Err(format!("Invalid chapter naming format: {}. Valid options: chapter-number-title, number-title, title-only, custom:pattern", input)),
        }
    }
}
