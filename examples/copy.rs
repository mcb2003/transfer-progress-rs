use std::{
    fs::File,
    io::{self, Read},
};

use transfer_progress::Transfer;

fn main() -> io::Result<()> {
    let reader = File::open("/dev/urandom")?.take(1024 * 1024 * 1024); // 1 GiB
    let writer = io::sink();

    // Create the transfer monitor
    let transfer = Transfer::new(reader, writer);

    while !transfer.is_complete() {
        std::thread::sleep(std::time::Duration::from_secs(1));
        // {:#} makes Transfer use SI units (MiB instead of MB)
        println!("{:#}", transfer);
    }

    // Catch any errors and retrieve the reader and writer
    let (_reader, _writer) = transfer.finish()?;
    Ok(())
}
