mod cli;
mod models;

use crate::models::FFProbeFormat;
use clap::Parser;
use crossterm::cursor::{MoveTo, MoveUp, RestorePosition, SavePosition};
use crossterm::{cursor, execute, ExecutableCommand};
use inflector::Inflector;
use std::io::{self, Cursor};
use std::path::{Path, PathBuf};
use std::process::Child;
use std::{
    io::BufRead,
    process::{Command, Stdio},
};

// ffmpeg command:
// ffmpeg -audible_key 92f0173bcfb2fc144897da04603d07c0
//        -audible_iv 33b00a91f553a54ee6c9e3556e021fab
//        -i The_Way_of_Kings_The_Stormlight_Archive_Book_1-AAX_22_64.aaxc
//        -map_metadata 0 -vn -codec:a mp3 the_way_of_kings.mp3

// ffprobe -i The_Way_of_Kings_The_Stormlight_Archive_Book_1-AAX_22_64.aaxc -print_format json -show_format

fn main() {
    let cli = cli::Cli::parse();

    let aaxc_file_path = cli.aaxc_path;
    let aaxc_file_path_stem = aaxc_file_path
        .file_stem()
        .expect("Could not get file stem, is the path a file?");

    // Take the aaxc file stem and add .voucher to it to get the voucher file path
    let voucher_file_path =
        aaxc_file_path.with_file_name(format!("{}.voucher", aaxc_file_path_stem.to_str().unwrap()));

    // Use serde to deserialize voucher file into `AudibleCliVoucher`
    let voucher: models::AudibleCliVoucher = serde_json::from_reader(
        std::fs::File::open(&voucher_file_path).expect("Failed to open voucher file"),
    )
    .expect("Failed to deserialize voucher file");

    let audible_key = voucher.content_license.license_response.key;
    let audible_iv = voucher.content_license.license_response.iv;

    let ffprobe_json = ffprobe(&aaxc_file_path);
    let title = ffprobe_json.format.tags.title;
    let album = ffprobe_json.format.tags.album;
    let duration = ffprobe_json.format.duration;

    let file_name = format!("{}.mp3", album.to_snake_case());

    println!("Title: {}", title);
    println!("File name: {}", file_name);

    let mut cmd = ffmpeg(aaxc_file_path, audible_key, audible_iv, duration, file_name);

    cmd.wait().unwrap();
}

fn ffprobe(aaxc_file_path: &Path) -> FFProbeFormat {
    let ffprobe_cmd = Command::new("ffprobe")
        .args([
            "-i",
            aaxc_file_path.to_str().unwrap(),
            "-print_format",
            "json",
            "-show_format",
            "-sexagesimal",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("failed to execute process");

    let ffprobe_output = std::str::from_utf8(&ffprobe_cmd.stdout).unwrap();
    let ffprobe_json: models::FFProbeFormat = serde_json::from_str(ffprobe_output).unwrap();
    ffprobe_json
}

fn ffmpeg(
    aaxc_file_path: PathBuf,
    audible_key: String,
    audible_iv: String,
    duration: String,
    file_name: String,
) -> Child {
    let mut cmd = Command::new("ffmpeg")
        .args([
            "-audible_key",
            audible_key.as_str(),
            "-audible_iv",
            audible_iv.as_str(),
            "-i",
            aaxc_file_path.to_str().unwrap(),
            "-progress",
            "/dev/stdout",
            "-y",
            "-map_metadata",
            "0",
            "-vn",
            "-codec:a",
            "mp3",
            file_name.as_str(),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to execute process");

    {
        let stdout = cmd.stdout.as_mut().unwrap();
        let stdout_reader = std::io::BufReader::new(stdout);
        let stdout_lines = stdout_reader.lines();

        for line in stdout_lines {
            let l = line.unwrap();
            if l.contains("time=") {
                println!("{} / {}", l, duration);
            }
            if l.contains("speed=") {
                println!("{}", l);
            }
        }
    }
    cmd
}
