# Transfer Progress

A small rust crate that allows you to monitor the speed, progress and estimated
completion time of a transfer between a reader and a writer.

Internally, this spins up a new thread for each transfer, and uses the
[progress-streams crate][progress-streams] to monitor it.

[progress-streams]: <https://crates.io/crates/progress-streams>

# Example

```rust
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
```
