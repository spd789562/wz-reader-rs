use wz_reader::ms::file::MsFile;

type Error = Box<dyn std::error::Error>;
type Result<T> = std::result::Result<T, Error>;

// usage:
//   cargo run --example parse_ms_file -- "path/to/file.ms"
//   cargo run --example parse_ms_file -- "D:\Path\To\file.ms"
fn main() -> Result<()> {
    let args = std::env::args_os().collect::<Vec<_>>();
    let base_path = args.get(1).expect("missing ms file path");

    let ms_file = MsFile::from_file(base_path, None)?;

    println!("{:?}", ms_file.header);

    Ok(())
}
