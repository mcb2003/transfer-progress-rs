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

    pub fn finish(self) -> io::Result<(R, W)> {
        self.handle.join().unwrap()
    }

    pub fn is_complete(&self) -> bool {
        // If someone would like to confirm the correctness of the ordering guarantees, that would
        // be much appreciated.
        self.state.complete.load(Ordering::Acquire)
    }

    pub fn transferred(&self) -> u64 {
        // If someone would like to confirm the correctness of the ordering guarantees, that would
        // be much appreciated.
        self.state.transferred.load(Ordering::Acquire)
    }

    pub fn running_time(&self) -> Duration {
        self.start_time.elapsed()
    }

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
    pub fn new(reader: R, writer: W, size: u64) -> Self {
        Self {
            inner: Transfer::new(reader, writer),
            size,
        }
    }

    pub fn fraction_transferred(&self) -> f64 {
        self.transferred() as f64 / self.size as f64
    }

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
