use clap::Parser;

mod chapters;
mod cli;
mod voucher;

fn main() {
    let cli = cli::Cli::parse();

    let aaxc_file_path = cli.aaxc_path;
    let aaxc_file_path_stem = aaxc_file_path
        .file_stem()
        .expect("Could not get file stem, is the path a file?");

    // Take the aaxc file stem and add .voucher to it to get the voucher file path
    let voucher_file_path =
        aaxc_file_path.with_file_name(format!("{}.voucher", aaxc_file_path_stem.to_str().unwrap()));

    println!("aaxc file path: {}", aaxc_file_path.display());
    println!("aaxc file exists? {}", aaxc_file_path.exists());
    println!("voucher file path: {}", voucher_file_path.display());
    println!("voucher file exists? {}", voucher_file_path.exists());
}
