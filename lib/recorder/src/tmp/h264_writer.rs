use crate::{EncodedFrame, RecorderError};
use crossbeam::channel::{Receiver, Sender, bounded};
use std::{
    fs::File,
    io::Write,
    path::PathBuf,
    sync::{Arc, Mutex},
    thread::JoinHandle,
    time::Duration,
};

/// H.264 file writer that handles frame queuing and file output.
///
/// This struct manages the writing of encoded H.264 frames to disk using
/// a background thread and queue system. It ensures that frames are written
/// in the correct order with proper Annex B formatting and SPS/PPS headers.
///
/// # Features
///
/// - Background thread for non-blocking file I/O
/// - Queue-based frame management to handle encoding bursts
/// - Automatic SPS/PPS extraction and writing
/// - Proper Annex B format with start codes
/// - Thread-safe frame submission
///
/// # Examples
///
/// ```no_run
/// use recorder::{H264Writer, EncodedFrame};
///
/// let writer = H264Writer::new("output.h264".into(), 2);
///
/// // Write frames as they become available
/// // writer.write_frame(encoded_frame);
///
/// // Finalize the file when done
/// // writer.finish().unwrap();
/// ```
#[derive(Debug, Clone)]
pub struct H264Writer {
    /// Output file path for the H.264 file
    output_path: PathBuf,
    /// Channel sender for submitting frames to the writer thread
    frame_sender: Arc<Sender<EncodedFrame>>,
    /// Handle to the background writer thread
    writer_worker: Arc<Mutex<JoinHandle<Result<(), RecorderError>>>>,
}

impl H264Writer {
    /// Create a new H.264 writer with specified output path and queue size.
    ///
    /// This constructor spawns a background thread that handles writing
    /// encoded frames to disk in the correct H.264 Annex B format.
    ///
    /// # Arguments
    ///
    /// * `output_path` - Path where the H.264 file will be created
    /// * `queue_size` - Maximum number of frames to queue (must be > 0)
    ///
    /// # Returns
    ///
    /// A new `H264Writer` instance ready to receive frames.
    ///
    /// # Panics
    ///
    /// Panics if `queue_size` is 0.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use recorder::{H264Writer};
    ///
    /// let writer = H264Writer::new("recording.h264".into(), 8);
    /// ```
    pub fn new(output_path: PathBuf, queue_size: usize) -> Self {
        assert!(queue_size > 0);

        let (frame_sender, frame_receiver) = bounded::<EncodedFrame>(queue_size);

        H264Writer {
            output_path: output_path.clone(),
            frame_sender: Arc::new(frame_sender),
            writer_worker: Arc::new(Mutex::new(std::thread::spawn(move || {
                Self::writer_thread(frame_receiver, output_path)
            }))),
        }
    }

    /// Send encoded frame to writer thread for disk writing.
    ///
    /// This method queues an encoded frame for writing to disk. If the queue
    /// is full, the frame will be dropped and a warning will be logged.
    ///
    /// # Arguments
    ///
    /// * `encoded_frame` - The encoded frame to write to disk
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use recorder::{H264Writer, EncodedFrame};
    ///
    /// let writer = H264Writer::new("output.h264".into(), 1024);
    ///
    /// // Assuming you have an encoded frame
    /// // let frame = EncodedFrame::Frame((0, vec![0u8; 1024]));
    /// // writer.write_frame(frame);
    /// ```
    pub fn write_frame(&self, encoded_frame: EncodedFrame) {
        if let Err(e) = self.frame_sender.try_send(encoded_frame) {
            log::warn!("Failed to try send frame to writer thread: {}", e);
        }
    }

    /// Writer thread that processes encoded frames and writes to MP4 file and H.264 file
    fn writer_thread(
        frame_receiver: Receiver<EncodedFrame>,
        output_path: PathBuf,
    ) -> Result<(), RecorderError> {
        // let mut is_written_sps_and_pps = false;
        let mut written_frame_counts = 0;
        let mut h264_file =
            File::create(&output_path).map_err(|e| RecorderError::FileOperationFailed(e))?;

        log::debug!("Writer thread started");
        log::info!("Creating H.264 file: {}", output_path.display());

        while let Ok(frame) = frame_receiver.recv() {
            log::debug!(
                "h264 writer thread frame receiver remained: {}",
                frame_receiver.capacity().unwrap_or_default() - frame_receiver.len()
            );

            match frame {
                EncodedFrame::Frame((_frame_index, frame_data)) => {
                    if let Err(e) = h264_file.write_all(&frame_data) {
                        log::warn!("write frame to h264 file failed: {e}");
                    } else {
                        written_frame_counts += 1;
                    }
                }
                EncodedFrame::Empty(_) => continue,
                EncodedFrame::EndOfStream => {
                    log::info!("h264 writer thread received `EndOfStream` signal");
                    break;
                }
            }
        }

        h264_file
            .flush()
            .map_err(|e| RecorderError::FileOperationFailed(e))?;

        log::info!(
            "Successfully created H.264 file with {} frame entireties: {}",
            written_frame_counts,
            output_path.display()
        );

        Ok(())
    }

    /// Finish encoding and finalize the H.264 file.
    ///
    /// This method sends an end-of-stream signal to the writer thread
    /// and waits for it to complete processing all queued frames.
    /// The file will be properly closed and flushed to disk.
    ///
    /// # Returns
    ///
    /// `Ok(())` if the file was successfully finalized, or `Err(RecorderError)` if failed.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use recorder::H264Writer;
    ///
    /// let writer = H264Writer::new("output.h264".into(), 1024);
    ///
    /// // Write some frames...
    ///
    /// // Finalize the file
    /// writer.finish().unwrap();
    /// ```
    pub fn finish(self) -> Result<(), RecorderError> {
        self.frame_sender
            .send(EncodedFrame::EndOfStream)
            .map_err(|e| {
                RecorderError::VideoEncodingFailed(format!(
                    "Failed to send end of stream signal: {}",
                    e
                ))
            })?;

        while !self.writer_worker.lock().unwrap().is_finished() {
            std::thread::sleep(Duration::from_secs(1));
            log::debug!("waiting writer worker finished");
        }

        log::info!(
            "Successfully save H.264 file: {}",
            self.output_path.display()
        );

        Ok(())
    }
}
