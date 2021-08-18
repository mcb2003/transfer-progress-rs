use std::{
    io::{self, prelude::*},
    sync::{Arc, atomic::{AtomicU64, Ordering}},
    thread,
};

use progress_streams::ProgressReader;

pub struct Transfer<R, W>
where
R: Read + Send + 'static,
W: Write + Send + 'static {
    transferred: Arc<AtomicU64>,
    handle: thread::JoinHandle<io::Result<(R, W)>>,
}

impl<R, W> Transfer<R, W>
where
R: Read + Send + 'static,
W: Write + Send + 'static {
    pub fn new(reader: R, mut writer: W) -> Self {
        let transferred = Arc::new(AtomicU64::new(0));
        let transferred_clone = Arc::clone(&transferred);
        let handle = thread::spawn(move || -> io::Result<(R, W)> {
                                   let mut reader = ProgressReader::new(reader, |bytes| {
                                                                    transferred_clone.fetch_add(bytes as u64, Ordering::Release);
                                   });
                                   io::copy(&mut reader, &mut writer)?;
                                   Ok((reader.into_inner(), writer))
        });
        Self { transferred, handle }
    }
}
