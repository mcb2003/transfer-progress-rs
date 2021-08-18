use std::{
    io::{self, prelude::*},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

use progress_streams::ProgressReader;

pub struct Transfer<R, W>
where
    R: Read + Send + 'static,
    W: Write + Send + 'static,
{
    start_time: Instant,
    transferred: Arc<AtomicU64>,
    handle: thread::JoinHandle<io::Result<(R, W)>>,
}

impl<R, W> Transfer<R, W>
where
    R: Read + Send + 'static,
    W: Write + Send + 'static,
{
    pub fn new(reader: R, mut writer: W) -> Self {
        let transferred = Arc::new(AtomicU64::new(0));
        let transferred_clone = Arc::clone(&transferred);
        let handle = thread::spawn(move || -> io::Result<(R, W)> {
            let mut reader = ProgressReader::new(reader, |bytes| {
                // If someone would like to confirm the correctness of the ordering guarantees, that would
                // be much appreciated.
                transferred_clone.fetch_add(bytes as u64, Ordering::Release);
            });
            io::copy(&mut reader, &mut writer)?;
            Ok((reader.into_inner(), writer))
        });
        Self {
            start_time: Instant::now(),
            transferred,
            handle,
        }
    }

    pub fn finish(self) -> io::Result<(R, W)> {
        self.handle.join().unwrap()
    }

    pub fn transferred(&self) -> u64 {
        // If someone would like to confirm the correctness of the ordering guarantees, that would
        // be much appreciated.
        self.transferred.load(Ordering::Acquire)
    }

    pub fn running_time(&self) -> Duration {
        self.start_time.elapsed()
    }

    pub fn speed(&self) -> u64 {
        (self.transferred() as f64 / self.running_time().as_secs_f64()).round() as u64
    }
}
