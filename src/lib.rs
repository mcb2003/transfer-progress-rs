#[cfg(feature = "bytesize")]
use std::fmt;
use std::{
    io::{self, prelude::*},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

#[cfg(feature = "bytesize")]
use bytesize::ByteSize;
use progress_streams::ProgressReader;

#[derive(Default)]
struct TransferState {
    transferred: AtomicU64,
    complete: AtomicBool,
}

/// Monitors the progress of a transfer from a [reader][Read] to a [writer][Write].
pub struct Transfer<R, W>
where
    R: Read + Send + 'static,
    W: Write + Send + 'static,
{
    start_time: Instant,
    state: Arc<TransferState>,
    handle: thread::JoinHandle<io::Result<(R, W)>>,
}

impl<R, W> Transfer<R, W>
where
    R: Read + Send + 'static,
    W: Write + Send + 'static,
{
    /// Creates and starts a new `Transfer`.
    /// # Example
    /// ```no_run
    /// use transfer_progress::Transfer;
    /// use std::fs::File;
    /// let reader = File::open("file1.txt")?;
    /// let writer = File::create("file2.txt")?;
    /// let transfer = Transfer::new(reader, writer);
    /// # Ok::<_, std::io::Error>(())
    /// ```
    pub fn new(reader: R, mut writer: W) -> Self {
        let state = Arc::new(TransferState::default());
        let state_clone = Arc::clone(&state);
        let handle = thread::spawn(move || -> io::Result<(R, W)> {
            let mut reader = ProgressReader::new(reader, |bytes| {
                // If someone would like to confirm the correctness of the ordering guarantees, that would
                // be much appreciated.
                state_clone
                    .transferred
                    .fetch_add(bytes as u64, Ordering::Release);
            });
            // We need to store the result and bubble it later so we can set the complete flag.
            let res = io::copy(&mut reader, &mut writer);
            state_clone.complete.store(true, Ordering::Release);
            res.map(|_| (reader.into_inner(), writer))
        });
        Self {
            start_time: Instant::now(),
            state,
            handle,
        }
    }

    /// Consumes the `Transfer`, blocking until the transfer is complete.
    ///
    /// If the transfer was successful, returns `Ok(reader, writer)`, otherwise returns
    /// the error.
    ///
    /// If the transfer is already complete, returns immediately.
    /// # Example
    /// ```no_run
    /// use transfer_progress::Transfer;
    /// use std::fs::File;
    /// let reader = File::open("file1.txt")?;
    /// let writer = File::create("file2.txt")?;
    /// let transfer = Transfer::new(reader, writer);
    /// let (reader, writer) = transfer.finish()?;
    /// # Ok::<_, std::io::Error>(())
    /// ```
    pub fn finish(self) -> io::Result<(R, W)> {
        self.handle.join().unwrap()
    }

    /// Tests if the transfer is complete
    /// # Example
    /// ```no_run
    /// use transfer_progress::Transfer;
    /// use std::fs::File;
    /// let reader = File::open("file1.txt")?;
    /// let writer = File::create("file2.txt")?;
    /// let transfer = Transfer::new(reader, writer);
    /// while !transfer.is_complete() {
    /// println!("Not complete yet");
    /// std::thread::sleep(std::time::Duration::from_secs(1));
    /// }
    /// println!("Complete!");
    /// # Ok::<_, std::io::Error>(())
    /// ```
    pub fn is_complete(&self) -> bool {
        // If someone would like to confirm the correctness of the ordering guarantees, that would
        // be much appreciated.
        self.state.complete.load(Ordering::Acquire)
    }

    /// Returns the number of bytes transferred thus far between the reader and the writer.
    /// # Example
    /// ```no_run
    /// use transfer_progress::Transfer;
    /// use std::fs::File;
    /// let reader = File::open("file1.txt")?;
    /// let writer = File::create("file2.txt")?;
    /// let transfer = Transfer::new(reader, writer);
    /// while !transfer.is_complete() {
    /// println!("{} bytes transferred so far", transfer.transferred());
    /// std::thread::sleep(std::time::Duration::from_secs(1));
    /// }
    /// println!("Complete!");
    /// # Ok::<_, std::io::Error>(())
    /// ```
    pub fn transferred(&self) -> u64 {
        // If someone would like to confirm the correctness of the ordering guarantees, that would
        // be much appreciated.
        self.state.transferred.load(Ordering::Acquire)
    }

    /// Returns the elapsed time since the transfer started.
    /// # Example
    /// ```no_run
    /// use transfer_progress::Transfer;
    /// use std::fs::File;
    /// let reader = File::open("file1.txt")?;
    /// let writer = File::create("file2.txt")?;
    /// let transfer = Transfer::new(reader, writer);
    /// while !transfer.is_complete() {}
    /// println!("Transfer took {:?}", transfer.running_time());
    /// # Ok::<_, std::io::Error>(())
    /// ```
    pub fn running_time(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Returns the average speed, in bytes per second, of the transfer.
    /// # Example
    /// ```no_run
    /// use transfer_progress::Transfer;
    /// use std::fs::File;
    /// let reader = File::open("file1.txt")?;
    /// let writer = File::create("file2.txt")?;
    /// let transfer = Transfer::new(reader, writer);
    /// while !transfer.is_complete() {
    /// println!("{}B/s", transfer.speed());
    /// std::thread::sleep(std::time::Duration::from_secs(1));
    /// }
    /// # Ok::<_, std::io::Error>(())
    /// ```
    pub fn speed(&self) -> u64 {
        (self.transferred() as f64 / self.running_time().as_secs_f64()).round() as u64
    }
}

#[cfg(feature = "bytesize")]
impl<R, W> fmt::Debug for Transfer<R, W>
where
    R: Read + Send + 'static,
    W: Write + Send + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let transferred = ByteSize::b(self.transferred());
        let speed = ByteSize::b(self.speed());
        if f.alternate() {
            // Use SI units
            write!(
                f,
                "{:#} ({:#}/s)",
                transferred.to_string_as(true),
                speed.to_string_as(true)
            )
        } else {
            write!(f, "{:#} ({:#}/s)", transferred, speed)
        }
    }
}

#[cfg(feature = "bytesize")]
impl<R, W> fmt::Display for Transfer<R, W>
where
    R: Read + Send + 'static,
    W: Write + Send + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

/// Monitors the progress of a transfer with a known size.
pub struct SizedTransfer<R, W>
where
    R: Read + Send + 'static,
    W: Write + Send + 'static,
{
    inner: Transfer<R, W>,
    size: u64,
}

impl<R, W> SizedTransfer<R, W>
where
    R: Read + Send + 'static,
    W: Write + Send + 'static,
{
    /// Creates and starts a new `SizedTransfer`.
    /// # Example
    /// ```no_run
    /// use transfer_progress::SizedTransfer;
    /// use std::fs::File;
    /// use std::io::Read;
    /// let reader = File::open("file1.txt")?.take(1024); // Bytes
    /// let writer = File::create("file2.txt")?;
    /// let transfer = SizedTransfer::new(reader, writer, 1024);
    /// # Ok::<_, std::io::Error>(())
    /// ```
    pub fn new(reader: R, writer: W, size: u64) -> Self {
        Self {
            inner: Transfer::new(reader, writer),
            size,
        }
    }

    /// Returns the total size (in bytes) of the transfer, as specified when calling
    /// [`new`][SizedTransfer::new].
    /// # Example
    /// ```no_run
    /// use transfer_progress::SizedTransfer;
    /// use std::fs::File;
    /// use std::io::Read;
    /// # fn main() -> std::io::Result<()> {
    /// let reader = File::open("file1.txt")?.take(1024); // Bytes
    /// let writer = File::create("file2.txt")?;
    /// let transfer = SizedTransfer::new(reader, writer, 1024);
    /// assert_eq!(transfer.size(), 1024);
    /// # Ok(())
    /// # }
    /// ```
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Returns the number of bytes remaining.
    /// # Example
    /// ```no_run
    /// use transfer_progress::SizedTransfer;
    /// use std::fs::File;
    /// use std::io::Read;
    /// let reader = File::open("file1.txt")?.take(1024); // Bytes
    /// let writer = File::create("file2.txt")?;
    /// let transfer = SizedTransfer::new(reader, writer, 1024);
    /// while !transfer.is_complete() {
    /// println!("{} bytes remaining", transfer.remaining());
    /// std::thread::sleep(std::time::Duration::from_secs(1));
    /// }
    /// # Ok::<_, std::io::Error>(())
    /// ```
    pub fn remaining(&self) -> u64 {
        self.size - self.inner.transferred()
    }

    /// Consumes the `SizedTransfer`, blocking until the transfer is complete.
    ///
    /// If the transfer was successful, returns `Ok(reader, writer)`, otherwise returns
    /// the error.
    ///
    /// If the transfer is already complete, returns immediately.
    /// # Example
    /// ```no_run
    /// use transfer_progress::SizedTransfer;
    /// use std::fs::File;
    /// use std::io::Read;
    /// let reader = File::open("file1.txt")?.take(1024); // Bytes
    /// let writer = File::create("file2.txt")?;
    /// let transfer = SizedTransfer::new(reader, writer, 1024);
    /// let (reader, writer) = transfer.finish()?;
    /// # Ok::<_, std::io::Error>(())
    /// ```
    pub fn finish(self) -> io::Result<(R, W)> {
        self.inner.finish()
    }

    /// Returns a fraction between 0.0 and 1.0 representing the state of the transfer.
    /// # Example
    /// ```no_run
    /// use transfer_progress::SizedTransfer;
    /// use std::fs::File;
    /// use std::io::Read;
    /// let reader = File::open("file1.txt")?.take(1024); // Bytes
    /// let writer = File::create("file2.txt")?;
    /// let transfer = SizedTransfer::new(reader, writer, 1024);
    /// while !transfer.is_complete() {
    /// println!("Transfer is {:.0}% complete", transfer.fraction_transferred() * 100.0);
    /// std::thread::sleep(std::time::Duration::from_secs(1));
    /// }
    /// # Ok::<_, std::io::Error>(())
    /// ```
    pub fn fraction_transferred(&self) -> f64 {
        self.transferred() as f64 / self.size as f64
    }

    /// Returns the approximate remaining time until this transfer completes. Returns `None` if
    /// this cannot be calculated (I.E. no bytes have been transferred yet, so a speed cannot be
    /// determined).
    /// # Example
    /// ```no_run
    /// use transfer_progress::SizedTransfer;
    /// use std::fs::File;
    /// use std::io::Read;
    /// let reader = File::open("file1.txt")?.take(1024); // Bytes
    /// let writer = File::create("file2.txt")?;
    /// let transfer = SizedTransfer::new(reader, writer, 1024);
    /// while !transfer.is_complete() {
    /// if let Some(eta) = transfer.eta() {
    /// println!("Transfer will complete in approximately {:?}", eta);
    /// } else {
    /// println!("Transfer completion time is unknown");
    /// }
    /// std::thread::sleep(std::time::Duration::from_secs(1));
    /// }
    /// # Ok::<_, std::io::Error>(())
    /// ```
    pub fn eta(&self) -> Option<Duration> {
        // Cache this so we don't have to perform an atomic access twice
        let transferred = self.inner.transferred();
        if transferred == 0 {
            return None;
        }
        let remaining = self.size - transferred;
        let elapsed = self.running_time().as_secs_f64();
        let eta = (elapsed / transferred as f64) * remaining as f64;
        Some(Duration::from_secs_f64(eta))
    }
}

impl<R, W> std::ops::Deref for SizedTransfer<R, W>
where
    R: Read + Send + 'static,
    W: Write + Send + 'static,
{
    type Target = Transfer<R, W>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

#[cfg(feature = "bytesize")]
impl<R, W> fmt::Debug for SizedTransfer<R, W>
where
    R: Read + Send + 'static,
    W: Write + Send + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let percentage = self.fraction_transferred() * 100.0;
        let transferred = ByteSize::b(self.transferred());
        let size = ByteSize::b(self.size);
        let speed = ByteSize::b(self.speed());
        if f.alternate() {
            write!(
                f,
                "{:.1} % ({} of {}, {}/s)",
                percentage, transferred, size, speed
            )
        } else {
            write!(
                f,
                "{:.1} % ({} of {}, {}/s)",
                percentage,
                transferred.to_string_as(true),
                size.to_string_as(true),
                speed.to_string_as(true)
            )
        }
    }
}

#[cfg(feature = "bytesize")]
impl<R, W> fmt::Display for SizedTransfer<R, W>
where
    R: Read + Send + 'static,
    W: Write + Send + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}
