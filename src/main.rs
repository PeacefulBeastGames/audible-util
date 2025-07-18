mod cli;
mod models;

use crate::models::FFProbeFormat;
use clap::Parser;
use inflector::Inflector;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::{
    io::BufRead,
    process::{Command, Stdio},
};
use anyhow::{Context, Result};
use log::{info, warn, error};
use indicatif::{ProgressBar, ProgressStyle};

fn main() -> Result<()> {
    // Initialize logger
    env_logger::init();

    info!("Starting audible-util");
    if let Err(e) = run() {
        error!("Fatal error: {e}");
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
    info!("audible-util finished successfully");
    Ok(())
}

fn run() -> Result<()> {
    let cli = cli::Cli::parse();

    info!("Parsing CLI arguments");

    // --- Early input validation ---

    // Check input .aaxc file exists, is readable, and has correct extension
    let aaxc_file_path = cli.aaxc_path;
    if !aaxc_file_path.exists() {
        anyhow::bail!(
            "Input file does not exist: {}. Please provide a valid .aaxc file.",
            aaxc_file_path.display()
        );
    }
    if !aaxc_file_path.is_file() {
        anyhow::bail!(
            "Input path is not a file: {}. Please provide a valid .aaxc file.",
            aaxc_file_path.display()
        );
    }
    if aaxc_file_path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()) != Some("aaxc".to_string()) {
        anyhow::bail!(
            "Input file does not have a .aaxc extension: {}. Please provide a valid Audible .aaxc file.",
            aaxc_file_path.display()
        );
    }
    if std::fs::File::open(&aaxc_file_path).is_err() {
        anyhow::bail!(
            "Input file is not readable: {}. Please check file permissions.",
            aaxc_file_path.display()
        );
    }

    // Determine voucher file path: use CLI override if provided
    let voucher_file_path = if let Some(voucher_path) = cli.voucher_path {
        info!("Using voucher file from CLI: {}", voucher_path.display());
        // Check voucher file exists and is readable
        if !voucher_path.exists() {
            anyhow::bail!(
                "Voucher file does not exist: {}. Please provide a valid voucher file.",
                voucher_path.display()
            );
        }
        if !voucher_path.is_file() {
            anyhow::bail!(
                "Voucher path is not a file: {}. Please provide a valid voucher file.",
                voucher_path.display()
            );
        }
        if std::fs::File::open(&voucher_path).is_err() {
            anyhow::bail!(
                "Voucher file is not readable: {}. Please check file permissions.",
                voucher_path.display()
            );
        }
        voucher_path
    } else {
        let aaxc_file_path_stem = aaxc_file_path
            .file_stem()
            .context("Could not get file stem from the input file path. Please provide a valid .aaxc file.")?;
        let path = aaxc_file_path.with_file_name(
            format!(
                "{}.voucher",
                aaxc_file_path_stem
                    .to_str()
                    .context("Failed to convert file stem to string. Please check your input file name.")?
            ),
        );
        info!("Using inferred voucher file: {}", path.display());
        if !path.exists() {
            anyhow::bail!(
                "Inferred voucher file does not exist: {}. Please provide a valid voucher file or use --voucher-path.",
                path.display()
            );
        }
        if !path.is_file() {
            anyhow::bail!(
                "Inferred voucher path is not a file: {}. Please provide a valid voucher file or use --voucher-path.",
                path.display()
            );
        }
        if std::fs::File::open(&path).is_err() {
            anyhow::bail!(
                "Inferred voucher file is not readable: {}. Please check file permissions or use --voucher-path.",
                path.display()
            );
        }
        path
    };

    // If output path is provided, check parent directory exists and is writable
    if let Some(ref output_path) = cli.output_path {
        use std::fs;
        use std::path::Path;
    
        if output_path.exists() && output_path.is_dir() {
            // If output_path is a directory, check if it's writable
            if std::fs::metadata(output_path)
                .map(|m| m.permissions().readonly())
                .unwrap_or(true)
            {
                anyhow::bail!(
                    "Output directory is not writable: {}. Please check permissions or specify a different output path.",
                    output_path.display()
                );
            }
        } else if let Some(parent) = output_path.parent() {
            // If output_path is a file path, check its parent directory
            if !parent.exists() {
                anyhow::bail!(
                    "Output directory does not exist: {}. Please create it or specify a different output path.",
                    parent.display()
                );
            }
            if std::fs::metadata(parent)
                .map(|m| m.permissions().readonly())
                .unwrap_or(true)
            {
                anyhow::bail!(
                    "Output directory is not writable: {}. Please check permissions or specify a different output path.",
                    parent.display()
                );
            }
        }
    }

    // --- Pre-flight checks for ffmpeg and ffprobe ---
    check_external_tool("ffmpeg")?;
    check_external_tool("ffprobe")?;

    // Use serde to deserialize voucher file into `AudibleCliVoucher`
    info!("Opening voucher file: {}", voucher_file_path.display());
    let voucher_file = std::fs::File::open(&voucher_file_path)
        .with_context(|| format!(
            "Failed to open voucher file: {}. Please ensure the file exists and is readable.",
            voucher_file_path.display()
        ))?;
    info!("Parsing voucher file");
    let voucher: models::AudibleCliVoucher = serde_json::from_reader(voucher_file)
        .with_context(|| format!(
            "Failed to parse voucher file: {}. Please ensure it is a valid JSON file generated by audible-cli.",
            voucher_file_path.display()
        ))?;
    voucher.validate().map_err(|e| anyhow::anyhow!("Invalid voucher: {e}"))?;
    info!("Voucher validated successfully");

    let audible_key = voucher.content_license.license_response.key;
    let audible_iv = voucher.content_license.license_response.iv;

    info!("Running ffprobe on input file: {}", aaxc_file_path.display());
    let ffprobe_json = ffprobe(&aaxc_file_path)
        .with_context(|| format!(
            "Failed to probe input file: {}. Please ensure ffprobe is installed and the file is a valid Audible AAXC file.",
            aaxc_file_path.display()
        ))?;
    ffprobe_json.validate().map_err(|e| anyhow::anyhow!("Invalid ffprobe data: {e}"))?;
    info!("ffprobe completed and validated");

    let title = ffprobe_json.format.tags.title;
    let album = ffprobe_json.format.tags.album;
    let duration = ffprobe_json.format.duration;

    // Determine output file extension and codec based on output_type (trait-based, extensible)
    use crate::cli::{OutputFormat, Mp3Format};
    let output_format: Box<dyn OutputFormat> = match cli.output_type {
        Some(ref t) => t.get_format(),
        None => Box::new(Mp3Format),
    };
    let codec = output_format.codec();
    let ext = output_format.extension();

    // Determine output file name: use CLI override if provided
    let file_name = if let Some(output_path) = cli.output_path {
        if output_path.exists() && output_path.is_dir() {
            // If output_path is a directory, use default filename inside it
            let default_name = format!("{}.{}", album.to_snake_case(), ext);
            output_path.join(default_name).to_string_lossy().to_string()
        } else {
            // If output_path is a file path (or does not exist), use as-is
            output_path.to_string_lossy().to_string()
        }
    } else {
        format!("{}.{}", album.to_snake_case(), ext)
    };

    if cli.split {
        warn!("--split is not yet supported and will be ignored.");
        println!("Warning: The --split option is not yet implemented.
Splitting output into chapters or segments is planned for a future release.
To add support, implement logic to parse chapter metadata and invoke ffmpeg for each segment,
using the OutputFormat trait for extensibility.
See the code comments for guidance on extending splitting functionality.");
    }

    info!("Title: {}", title);
    info!("Output file name: {}", file_name);

    info!("Starting ffmpeg conversion");
    let mut cmd = ffmpeg(
        aaxc_file_path,
        audible_key,
        audible_iv,
        duration,
        file_name.clone(),
        codec,
    )
    .with_context(|| {
        "Failed to start ffmpeg. Please ensure ffmpeg is installed and available in your PATH."
    })?;

    let status = cmd.wait()
        .with_context(|| "ffmpeg process failed to complete. Please check your input files and try again.")?;

    if status.success() {
        info!("ffmpeg conversion completed successfully");
    } else {
        error!("ffmpeg conversion failed with status: {:?}", status);
        anyhow::bail!(
            "ffmpeg failed to convert the file. Please check your input files and try again. \
If the problem persists, ensure that ffmpeg is installed and supports the required codecs."
        );
    }

    Ok(())
}

fn ffprobe(aaxc_file_path: &Path) -> Result<FFProbeFormat> {
    let ffprobe_cmd = Command::new("ffprobe")
        .args([
            "-i",
            aaxc_file_path
                .to_str()
                .context("Failed to convert input file path to string.")?,
            "-print_format",
            "json",
            "-show_format",
            "-sexagesimal",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| "Failed to execute ffprobe. Is ffprobe installed and available in your PATH?")?;

    if !ffprobe_cmd.status.success() {
        let stderr = String::from_utf8_lossy(&ffprobe_cmd.stderr);
        anyhow::bail!(
            "ffprobe failed with error:\n{}\nPlease ensure the input file is a valid Audible AAXC file.",
            stderr
        );
    }

    let ffprobe_output = std::str::from_utf8(&ffprobe_cmd.stdout)
        .context("Failed to parse ffprobe output as UTF-8.")?;
    let ffprobe_json: models::FFProbeFormat = serde_json::from_str(ffprobe_output)
        .context("Failed to parse ffprobe output as JSON. The file may not be a valid Audible AAXC file.")?;
    Ok(ffprobe_json)
}

fn ffmpeg(
    aaxc_file_path: PathBuf,
    audible_key: String,
    audible_iv: String,
    duration: String,
    file_name: String,
    codec: &str,
) -> Result<Child> {
    let mut cmd = Command::new("ffmpeg")
        .args([
            "-audible_key",
            audible_key.as_str(),
            "-audible_iv",
            audible_iv.as_str(),
            "-i",
            aaxc_file_path
                .to_str()
                .context("Failed to convert input file path to string.")?,
            "-progress",
            "/dev/stdout",
            "-y",
            "-map_metadata",
            "0",
            "-vn",
            "-codec:a",
            codec,
            file_name.as_str(),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| "Failed to execute ffmpeg. Is ffmpeg installed and available in your PATH?")?;

    {
        let stdout = cmd.stdout.as_mut().context("Failed to capture ffmpeg stdout.")?;
        let stdout_reader = std::io::BufReader::new(stdout);

        // Progress bar setup
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner} [{elapsed_precise}] {msg}")
                .unwrap()
        );
        pb.enable_steady_tick(std::time::Duration::from_millis(100));

        for line in stdout_reader.lines() {
            let l = line.context("Failed to read line from ffmpeg output.")?;
            if l.contains("time=") {
                pb.set_message(format!("Progress: {} / {}", l, duration));
            }
            if l.contains("speed=") {
                pb.set_message(format!("{} | {}", pb.message(), l));
            }
        }
        pb.finish_with_message("Conversion complete");
    }
    info!("ffmpeg process finished");
    Ok(cmd)
}

/// Checks if an external tool is available in PATH, returns error with guidance if not.
fn check_external_tool(tool: &str) -> Result<()> {
    if which::which(tool).is_err() {
        anyhow::bail!(
            "Required external tool '{}' is not installed or not found in your PATH.\n\
            Please install '{}' and ensure it is available in your system PATH.\n\
            See the README for installation instructions.",
            tool, tool
        );
    }
    Ok(())
}
