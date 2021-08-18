use std::{
    fs::File,
    io::{self, Read},
};

use transfer_progress::SizedTransfer;

/// 1 GiB
const DATA_TO_TRANSFER: u64 = 1024 * 1024 * 1024;

fn main() -> io::Result<()> {
    let reader = File::open("/dev/urandom")?.take(DATA_TO_TRANSFER);
    let writer = io::sink();

    // Create the transfer monitor
    let transfer = SizedTransfer::new(reader, writer, DATA_TO_TRANSFER);

    while !transfer.is_complete() {
        std::thread::sleep(std::time::Duration::from_secs(1));
        // {:#} makes Transfer use SI units (MiB instead of MB)
        println!("{:#}", transfer);
    }

    // Catch any errors and retrieve the reader and writer
    let (_reader, _writer) = transfer.finish()?;
    Ok(())
}
