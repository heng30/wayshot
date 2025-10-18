use crate::{FPS, recorder::ResizedImageBuffer, recorder_error::RecorderError};
use x264::{Colorspace, Data, Encoder, Image, Preset, Setup, Tune};

/// Represents the result of video encoding operations.
///
/// This enum is used to communicate different types of encoded frame data
/// through the recording pipeline, including actual frames, empty frames,
/// and end-of-stream markers.
///
/// # Examples
///
/// ```no_run
/// use recorder::{VideoEncoder, EncodedFrame, FPS};
///
/// let mut encoder = VideoEncoder::new(1920, 1080, FPS::Fps30).unwrap();
/// // Assuming you have a frame to encode
/// let encoded = encoder.encode_frame(/* frame */).unwrap();
///
/// match encoded {
///     EncodedFrame::Frame((index, data)) => {
///         println!("Encoded frame {} with {} bytes", index, data.len());
///     }
///     EncodedFrame::Empty(index) => {
///         println!("Empty frame at index {}", index);
///     }
///     EncodedFrame::EndOfStream => {
///         println!("End of stream reached");
///     }
/// }
/// ```
#[derive(Debug, Clone)]
pub enum EncodedFrame {
    /// Empty frame placeholder with frame index
    Empty(u64),
    /// Encoded frame data with frame index and H.264 bytes
    Frame((u64, Vec<u8>)),
    /// End of stream marker
    EndOfStream,
}

impl Default for EncodedFrame {
    /// Create a default empty frame with index 0
    fn default() -> Self {
        EncodedFrame::Empty(0)
    }
}

/// H.264 video encoder with streaming capabilities for screen recording.
///
/// This struct provides real-time H.264 video encoding using the x264 library.
/// It's optimized for screen recording with high performance and quality.
///
/// # Features
///
/// - Real-time H.264 encoding with x264
/// - Optimized for screen content with high performance
/// - Support for multiple frame rates and resolutions
/// - Quality-focused encoding with minimal frame skipping
///
/// # Examples
///
/// ```no_run
/// use recorder::{VideoEncoder, FPS};
///
/// // Create encoder for 1080p at 30 FPS
/// let mut encoder = VideoEncoder::new(1920, 1080, FPS::Fps30).unwrap();
///
/// // Encode frames in a loop
/// // let encoded_frame = encoder.encode_frame(frame).unwrap();
/// ```
pub struct VideoEncoder {
    /// Video width in pixels
    width: u32,
    /// Video height in pixels
    height: u32,
    /// Current frame index
    frame_index: u64,
    /// x264 encoder instance
    encoder: Encoder,
    /// Frames per second
    fps: FPS,
}

impl VideoEncoder {
    /// Get mutable reference to the underlying x264 encoder.
    ///
    /// This method provides direct access to the x264 encoder instance
    /// for advanced configuration or operations not covered by the high-level API.
    ///
    /// # Returns
    ///
    /// Mutable reference to the underlying `Encoder` instance.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use recorder::{VideoEncoder, FPS};
    ///
    /// let mut encoder = VideoEncoder::new(1920, 1080, FPS::Fps30).unwrap();
    /// let x264_encoder = encoder.encoder();
    /// // Use x264 encoder directly for advanced operations
    /// ```
    pub fn encoder(&mut self) -> &mut Encoder {
        &mut self.encoder
    }

    /// Get encoder headers (SPS and PPS) for writing to file.
    ///
    /// This method returns the sequence parameter set (SPS) and picture
    /// parameter set (PPS) headers that must be written at the beginning
    /// of the H.264 file for proper decoding.
    ///
    /// # Returns
    ///
    /// `Ok(Data)` containing the header data, or `Err(RecorderError)` if failed.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use recorder::{VideoEncoder, FPS};
    ///
    /// let mut encoder = VideoEncoder::new(1920, 1080, FPS::Fps30).unwrap();
    /// let headers = encoder.headers().unwrap();
    /// // Write headers to file before encoding frames
    /// ```
    pub fn headers(&mut self) -> Result<Data<'_>, RecorderError> {
        self.encoder.headers().map_err(|e| {
            RecorderError::VideoEncodingFailed(format!("Failed to get encoder headers: {:?}", e))
        })
    }

    /// Flush the encoder to process any delayed frames.
    ///
    /// This method flushes the encoder's internal buffer and returns any
    /// remaining encoded frames. This should be called after all frames
    /// have been encoded to ensure all data is processed.
    ///
    /// # Returns
    ///
    /// The flush iterator for processing remaining frames, or `Err(RecorderError)` if failed.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use recorder::{VideoEncoder, FPS};
    ///
    /// let mut encoder = VideoEncoder::new(1920, 1080, FPS::Fps30).unwrap();
    /// // Encode all frames...
    /// let mut flush = encoder.flush().unwrap();
    /// while let Some(result) = flush.next() {
    ///     let (data, _) = result.unwrap();
    ///     // Write remaining frames to file
    /// }
    /// ```
    pub fn flush(self) -> Result<x264::Flush, RecorderError> {
        Ok(self.encoder.flush())
    }

    /// Create a new video encoder with specified dimensions and frame rate.
    ///
    /// This constructor sets up an x264 encoder optimized for screen recording
    /// with high performance and quality settings.
    ///
    /// # Arguments
    ///
    /// * `width` - Video width in pixels (must be greater than 0)
    /// * `height` - Video height in pixels (must be greater than 0)
    /// * `fps` - Frame rate for the recording
    ///
    /// # Returns
    ///
    /// `Ok(VideoEncoder)` if initialization succeeded, or `Err(RecorderError)` if failed.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use recorder::{VideoEncoder, FPS};
    ///
    /// // Create encoder for 1080p at 30 FPS
    /// let encoder = VideoEncoder::new(1920, 1080, FPS::Fps30).unwrap();
    ///
    /// // Create encoder for 4K at 60 FPS
    /// let encoder = VideoEncoder::new(3840, 2160, FPS::Fps60).unwrap();
    /// ```
    pub fn new(width: u32, height: u32, fps: FPS) -> Result<Self, RecorderError> {
        assert!(width > 0 && height > 0);

        // Create x264 encoder with optimized settings for screen recording
        // Using ultrafast preset with stillimage tune for minimal memory usage
        let encoder = Setup::preset(
            Preset::Ultrafast, // Minimal memory usage and fastest encoding
            Tune::StillImage,  // Optimized for screen content (static images)
            true,              // fast_decode: Enable fast decoding
            true,              // zero_latency: Minimal internal buffering
        )
        .fps(fps.to_u32(), 1)
        .baseline() // Ensure maximum compatibility
        .build(Colorspace::RGB, width as i32, height as i32)
        .map_err(|e| {
            RecorderError::VideoEncodingFailed(format!("Failed to create x264 encoder: {:?}", e))
        })?;

        Ok(Self {
            encoder,
            width,
            height,
            frame_index: 0,
            fps,
        })
    }

    /// Encode a single frame to H.264 format.
    ///
    /// This method takes a resized RGB image buffer and encodes it to H.264 format
    /// using the x264 encoder.
    ///
    /// # Arguments
    ///
    /// * `img` - Resized image buffer in RGB format
    ///
    /// # Returns
    ///
    /// `Ok(EncodedFrame)` containing the encoded frame data and index,
    /// or `Err(RecorderError)` if encoding failed.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use recorder::{VideoEncoder, FPS};
    ///
    /// let mut encoder = VideoEncoder::new(1920, 1080, FPS::Fps30).unwrap();
    ///
    /// // Assuming you have a frame to encode
    /// // let frame = /* get frame from somewhere */;
    /// // let encoded = encoder.encode_frame(frame).unwrap();
    ///
    /// match encoded {
    ///     EncodedFrame::Frame((index, data)) => {
    ///         println!("Encoded frame {} with {} bytes", index, data.len());
    ///     }
    ///     _ => unreachable!(),
    /// }
    /// ```
    pub fn encode_frame(&mut self, img: ResizedImageBuffer) -> Result<EncodedFrame, RecorderError> {
        let (img_width, img_height) = img.dimensions();
        if img_width != self.width || img_height != self.height {
            return Err(RecorderError::ImageProcessingFailed(format!(
                "frame is already resize. current size: {}x{}. expect size: {}x{}",
                img_width, img_height, self.width, self.height
            )));
        }

        // Create x264 image from RGB buffer
        let image = Image::rgb(self.width as i32, self.height as i32, img.as_raw());

        // Calculate timestamp in milliseconds (frame_index * 1000 / fps)
        let timestamp_ms = (self.frame_index * 1000) / self.fps.to_u32() as u64;

        let (data, _) = self
            .encoder
            .encode(timestamp_ms as i64, image)
            .map_err(|e| {
                RecorderError::VideoEncodingFailed(format!("x264 encoding failed: {:?}", e))
            })?;

        let encoded_data = data.entirety().to_vec();
        let encoded_frame = EncodedFrame::Frame((self.frame_index, encoded_data));
        self.frame_index += 1;
        Ok(encoded_frame)
    }
}
