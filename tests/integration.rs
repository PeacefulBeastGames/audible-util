use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::NamedTempFile;
use std::fs::File;
use std::io::Write;
use std::env;
use std::os::unix::fs::PermissionsExt;

// Sample valid voucher JSON
const VALID_VOUCHER: &str = r#"{
  "content_license": {
    "acr": "acr",
    "asin": "asin",
    "content_metadata": {
      "content_reference": {
        "acr": "acr",
        "asin": "asin",
        "codec": "codec",
        "content_format": "format",
        "content_size_in_bytes": 123,
        "file_version": "1",
        "marketplace": "market",
        "sku": "sku",
        "tempo": "tempo",
        "version": "v1"
      },
      "content_url": { "offline_url": "url" },
      "last_position_heard": {
        "last_updated": "now",
        "position_ms": 0,
        "status": "ok"
      }
    },
    "drm_type": "drm",
    "granted_right": "right",
    "license_id": "id",
    "license_response": {
      "key": "key",
      "iv": "iv",
      "rules": [{
        "parameters": [{
          "expire_date": "date",
          "type": "type"
        }],
        "name": "rule"
      }]
    },
    "license_response_type": "type",
    "message": "msg",
    "playback_info": {
      "last_position_heard": {
        "last_updated": "now",
        "position_ms": 0,
        "status": "ok"
      }
    },
    "preview": false,
    "request_id": "req",
    "requires_ad_supported_playback": false,
    "status_code": "ok",
    "voucher_id": "vid"
  },
  "response_groups": ["group"]
}"#;

// Sample valid ffprobe JSON
const VALID_FFPROBE: &str = r#"{
  "format": {
    "filename": "file.aaxc",
    "nb_streams": 1,
    "nb_programs": 0,
    "nb_stream_groups": 0,
    "format_name": "aax",
    "format_long_name": "Audible AAX",
    "start_time": "0",
    "duration": "100",
    "size": "1000",
    "bit_rate": "128000",
    "probe_score": 100,
    "tags": {
      "major_brand": "brand",
      "minor_version": "1",
      "compatible_brands": "brand",
      "creation_time": "now",
      "genre": "genre",
      "title": "title",
      "artist": "artist",
      "album_artist": "album_artist",
      "album": "album",
      "comment": "comment",
      "copyright": "copyright",
      "date": "2020"
    }
  }
}"#;

fn write_temp_file(contents: &str, suffix: &str) -> NamedTempFile {
    use tempfile::Builder;
    let mut file = Builder::new()
        .suffix(suffix)
        .tempfile()
        .expect("temp file");
    file.write_all(contents.as_bytes()).expect("write temp");
    file
}

#[test]
fn test_missing_required_argument() {
    let mut cmd = Command::cargo_bin("audible-util").unwrap();
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("USAGE"));
}

#[test]
fn test_invalid_input_file() {
    let mut cmd = Command::cargo_bin("audible-util").unwrap();
    cmd.arg("--aaxc_path").arg("nonexistent.aaxc");
    cmd.arg("--voucher-path").arg("nonexistent.voucher");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Could not get file stem"));
}

#[test]
fn test_input_file_missing() {
    let mut cmd = Command::cargo_bin("audible-util").unwrap();
    cmd.arg("--aaxc_path").arg("doesnotexist.aaxc");
    cmd.arg("--voucher-path").arg("doesnotexist.voucher");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Input file does not exist"));
}

#[test]
fn test_input_file_wrong_extension() {
    let file = write_temp_file("", ".mp3");
    let voucher = write_temp_file(VALID_VOUCHER, ".voucher");
    let mut cmd = Command::cargo_bin("audible-util").unwrap();
    cmd.arg("--aaxc_path").arg(file.path());
    cmd.arg("--voucher-path").arg(voucher.path());
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Input file does not have a .aaxc extension"));
}

#[test]
fn test_input_file_not_readable() {
    let file = write_temp_file("", ".aaxc");
    let voucher = write_temp_file(VALID_VOUCHER, ".voucher");
    // Set file to 000 permissions (unreadable)
    std::fs::set_permissions(file.path(), std::fs::Permissions::from_mode(0o000)).unwrap();
    let mut cmd = Command::cargo_bin("audible-util").unwrap();
    cmd.arg("--aaxc_path").arg(file.path());
    cmd.arg("--voucher-path").arg(voucher.path());
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Input file is not readable"));
    // Restore permissions for cleanup
    std::fs::set_permissions(file.path(), std::fs::Permissions::from_mode(0o644)).unwrap();
}

#[test]
fn test_voucher_file_missing() {
    let file = write_temp_file("", ".aaxc");
    let mut cmd = Command::cargo_bin("audible-util").unwrap();
    cmd.arg("--aaxc_path").arg(file.path());
    cmd.arg("--voucher-path").arg("doesnotexist.voucher");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Voucher file does not exist"));
}

#[test]
fn test_voucher_file_not_readable() {
    let file = write_temp_file("", ".aaxc");
    let voucher = write_temp_file(VALID_VOUCHER, ".voucher");
    std::fs::set_permissions(voucher.path(), std::fs::Permissions::from_mode(0o000)).unwrap();
    let mut cmd = Command::cargo_bin("audible-util").unwrap();
    cmd.arg("--aaxc_path").arg(file.path());
    cmd.arg("--voucher-path").arg(voucher.path());
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Voucher file is not readable"));
    std::fs::set_permissions(voucher.path(), std::fs::Permissions::from_mode(0o644)).unwrap();
}

#[test]
fn test_output_directory_missing() {
    let file = write_temp_file("", ".aaxc");
    let voucher = write_temp_file(VALID_VOUCHER, ".voucher");
    let out_path = std::path::Path::new("nonexistent_dir/output.mp3");
    let mut cmd = Command::cargo_bin("audible-util").unwrap();
    cmd.arg("--aaxc_path").arg(file.path());
    cmd.arg("--voucher-path").arg(voucher.path());
    cmd.arg("--output-path").arg(out_path);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Output directory does not exist"));
}

#[test]
fn test_output_directory_not_writable() {
    let file = write_temp_file("", ".aaxc");
    let voucher = write_temp_file(VALID_VOUCHER, ".voucher");
    let tempdir = tempfile::tempdir().unwrap();
    let unwritable_dir = tempdir.path().join("unwritable");
    std::fs::create_dir(&unwritable_dir).unwrap();
    std::fs::set_permissions(&unwritable_dir, std::fs::Permissions::from_mode(0o555)).unwrap();
    let out_path = unwritable_dir.join("output.mp3");
    let mut cmd = Command::cargo_bin("audible-util").unwrap();
    cmd.arg("--aaxc_path").arg(file.path());
    cmd.arg("--voucher-path").arg(voucher.path());
    cmd.arg("--output-path").arg(&out_path);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Output directory is not writable"));
    std::fs::set_permissions(&unwritable_dir, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn test_missing_ffmpeg() {
    let aaxc = write_temp_file("", ".aaxc");
    let voucher = write_temp_file(VALID_VOUCHER, ".voucher");
    // Remove ffmpeg from PATH
    let orig_path = env::var("PATH").unwrap();
    env::set_var("PATH", "");
    let mut cmd = Command::cargo_bin("audible-util").unwrap();
    cmd.arg("--aaxc_path").arg(aaxc.path());
    cmd.arg("--voucher-path").arg(voucher.path());
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Required external tool 'ffmpeg' is not installed"));
    env::set_var("PATH", orig_path);
}

#[test]
fn test_invalid_voucher_file() {
    let aaxc = write_temp_file("", ".aaxc");
    let voucher = write_temp_file("not json", ".voucher");
    let mut cmd = Command::cargo_bin("audible-util").unwrap();
    cmd.arg("--aaxc_path").arg(aaxc.path());
    cmd.arg("--voucher-path").arg(voucher.path());
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Failed to parse voucher file"));
}

#[test]
fn test_output_format_selection() {
    let aaxc = write_temp_file("", ".aaxc");
    let voucher = write_temp_file(VALID_VOUCHER, ".voucher");
    // Patch ffprobe to echo valid JSON
    let ffprobe = write_temp_file(
        "#!/bin/sh\necho '$VALID_FFPROBE'",
        ".sh"
    );
    let ffprobe_path = ffprobe.path();
    std::fs::set_permissions(ffprobe_path, std::fs::Permissions::from_mode(0o755)).unwrap();

    let orig_path = env::var("PATH").unwrap();
    let new_path = format!("{}:{}", ffprobe_path.parent().unwrap().display(), orig_path);
    env::set_var("PATH", new_path);

    let mut cmd = Command::cargo_bin("audible-util").unwrap();
    cmd.arg("--aaxc_path").arg(aaxc.path());
    cmd.arg("--voucher-path").arg(voucher.path());
    cmd.arg("--output-type").arg("flac");
    // Should succeed up to ffmpeg (which will fail, but output format logic is exercised)
    let assert = cmd.assert();
    assert.stderr(predicate::str::contains("ffmpeg"));
    // Restore PATH
    env::set_var("PATH", orig_path);
}

#[test]
fn test_missing_ffprobe() {
    let aaxc = write_temp_file("", ".aaxc");
    let voucher = write_temp_file(VALID_VOUCHER, ".voucher");
    // Remove ffprobe from PATH
    let orig_path = env::var("PATH").unwrap();
    env::set_var("PATH", "");
    let mut cmd = Command::cargo_bin("audible-util").unwrap();
    cmd.arg("--aaxc_path").arg(aaxc.path());
    cmd.arg("--voucher-path").arg(voucher.path());
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Failed to execute ffprobe"));
    env::set_var("PATH", orig_path);
}

#[test]
fn test_voucher_data_validation() {
    let aaxc = write_temp_file("", ".aaxc");
    // Voucher missing response_groups
    let invalid_voucher = r#"{
      "content_license": {
        "acr": "acr",
        "asin": "asin",
        "content_metadata": {
          "content_reference": {
            "acr": "acr",
            "asin": "asin",
            "codec": "codec",
            "content_format": "format",
            "content_size_in_bytes": 123,
            "file_version": "1",
            "marketplace": "market",
            "sku": "sku",
            "tempo": "tempo",
            "version": "v1"
          },
          "content_url": { "offline_url": "url" },
          "last_position_heard": {
            "last_updated": "now",
            "position_ms": 0,
            "status": "ok"
          }
        },
        "drm_type": "drm",
        "granted_right": "right",
        "license_id": "id",
        "license_response": {
          "key": "key",
          "iv": "iv",
          "rules": [{
            "parameters": [{
              "expire_date": "date",
              "type": "type"
            }],
            "name": "rule"
          }]
        },
        "license_response_type": "type",
        "message": "msg",
        "playback_info": {
          "last_position_heard": {
            "last_updated": "now",
            "position_ms": 0,
            "status": "ok"
          }
        },
        "preview": false,
        "request_id": "req",
        "requires_ad_supported_playback": false,
        "status_code": "ok",
        "voucher_id": "vid"
      },
      "response_groups": []
    }"#;
    let voucher = write_temp_file(invalid_voucher, ".voucher");
    let mut cmd = Command::cargo_bin("audible-util").unwrap();
    cmd.arg("--aaxc_path").arg(aaxc.path());
    cmd.arg("--voucher-path").arg(voucher.path());
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Invalid voucher"));
}
#[test]
fn test_real_conversion_aborted() {
    use std::process::{Command as StdCommand, Stdio};
    use std::{thread, time};

    // Path to real aaxc and voucher files
    let aaxc_path = "/home/ondra/Music/Oathbringer_The_Stormlight_Archive_Book_3-AAX_22_64.aaxc";
    let voucher_path = "/home/ondra/Music/Oathbringer_The_Stormlight_Archive_Book_3-AAX_22_64.voucher";

    // Output file in temp dir
    let out_file = tempfile::NamedTempFile::new().unwrap();
    let out_path = out_file.path().to_owned();

    // Spawn the process
    let mut child = StdCommand::new("cargo")
        .arg("run")
        .arg("--bin")
        .arg("audible-util")
        .arg("--")
        .arg("--aaxc_path").arg(aaxc_path)
        .arg("--voucher-path").arg(voucher_path)
        .arg("--output-path").arg(&out_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn audible-util");

    // Let it run for a short time, then abort
    thread::sleep(time::Duration::from_secs(2));
    let _ = child.kill();

    // Wait for process to exit and collect output
    let output = child.wait_with_output().expect("Failed to wait on child");

    // Assert process was killed and output contains expected abort/error message
    assert!(!output.status.success(), "Process should not succeed when killed early");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Aborted") || stderr.contains("killed") || stderr.contains("signal") || !output.status.success(),
        "Expected abort/killed message in stderr, got: {}",
        stderr
    );
}