use crate::command::ControlCommand;
use eframe::egui;
use opencv::{
    core, imgproc,
    prelude::*,
    videoio::{self, VideoCapture},
};
use std::{path::Path, sync::mpsc, thread};

// Defines the error types.
#[derive(Debug, Clone)]
pub enum VideoReaderError {
    OpenCV(String),
}

/// A struct responsible for opening a video and decoding it frame by frame.
/// This struct does not handle threading.
pub struct FrameDecoder {
    cap: VideoCapture,
}

impl FrameDecoder {
    /// Creates a new FrameDecoder by opening the specified video file.
    pub fn new(path: &Path) -> Result<Self, VideoReaderError> {
        let path_str = path.to_str().unwrap_or_default();
        let cap = VideoCapture::from_file(path_str, videoio::CAP_ANY)
            .map_err(|e| VideoReaderError::OpenCV(format!("Failed to open video file: {:?}", e)))?;

        // Check if the video capture was actually opened successfully.
        if !cap.is_opened().unwrap_or(false) {
            return Err(VideoReaderError::OpenCV(format!(
                "Failed to open video file: {}",
                path_str
            )));
        }

        Ok(Self { cap })
    }

    /// Returns the frames per second (FPS) of the video.
    pub fn get_fps(&self) -> f64 {
        self.cap.get(videoio::CAP_PROP_FPS).unwrap_or(30.0)
    }

    /// Returns the width of the video frames.
    pub fn width(&self) -> u32 {
        self.cap.get(videoio::CAP_PROP_FRAME_WIDTH).unwrap_or(0.0) as u32
    }

    /// Returns the height of the video frames.
    pub fn height(&self) -> u32 {
        self.cap.get(videoio::CAP_PROP_FRAME_HEIGHT).unwrap_or(0.0) as u32
    }

    /// Returns the total duration of the video in seconds.
    pub fn duration(&self) -> f64 {
        let frame_count = self.cap.get(videoio::CAP_PROP_FRAME_COUNT).unwrap_or(0.0);
        let fps = self.get_fps();
        if fps > 0.0 {
            frame_count / fps
        } else {
            0.0
        }
    }

    /// Reads the next frame from the video and returns it as a ColorImage.
    /// Returns `Ok(None)` if the end of the video is reached.
    // e.g., -> Result<Option<(egui::ColorImage, f64)>, VideoReaderError>
    // タイムスタンプは self.cap.get(videoio::CAP_PROP_POS_MSEC)? で取得
    pub fn read_next_frame(
        &mut self,
    ) -> Result<Option<(egui::ColorImage, f64)>, VideoReaderError> {
        let mut frame = core::Mat::default();
        match self.cap.read(&mut frame) {
            Ok(true) if !frame.empty() => {
                let timestamp_ms = self.cap.get(videoio::CAP_PROP_POS_MSEC).unwrap_or(0.0);

                // Convert from OpenCV's BGR format to egui's RGB format.
                let mut rgb_frame = core::Mat::default();
                imgproc::cvt_color(
                    &frame,
                    &mut rgb_frame,
                    imgproc::COLOR_BGR2RGB,
                    0,
                    core::AlgorithmHint::ALGO_HINT_DEFAULT,
                )
                .map_err(|e| {
                    VideoReaderError::OpenCV(format!("Failed to convert frame to RGB: {}", e))
                })?;

                // Convert Mat data to egui::ColorImage.
                let size = rgb_frame.size().expect("Failed to get frame size");
                let data = unsafe {
                    std::slice::from_raw_parts(
                        rgb_frame.data(),
                        size.width as usize * size.height as usize * 3,
                    )
                };
                let color_image =
                    egui::ColorImage::from_rgb([size.width as usize, size.height as usize], data);

                Ok(Some((color_image, timestamp_ms)))
            }
            // End of video or read error
            _ => Ok(None),
        }
    }
}

pub struct VideoReader {
    // Holds the handle of the spawned thread.
    _thread_handle: thread::JoinHandle<()>,
    pub width: u32,
    pub height: u32,
    pub duration: f64,
}

impl VideoReader {
    /// Creates a new VideoReader and starts reading the video on a background thread.
    ///
    /// # Arguments
    /// * `path` - The path to the video file to read.
    /// * `image_sender` - The sender to send the read frames (`egui::ColorImage`) to the UI thread.
    /// * `control_receiver` - The receiver for control commands from the UI thread.
    ///
    /// # Returns
    /// * `Ok(Self)` - If the thread was successfully started.
    /// * `Err(VideoReaderError)` - If opening the video file fails.
    pub fn new(
        path: &Path,
        image_sender: mpsc::Sender<Result<(egui::ColorImage, f64), VideoReaderError>>,
        control_receiver: mpsc::Receiver<ControlCommand>,
    ) -> Result<Self, VideoReaderError> {
        let mut decoder = FrameDecoder::new(path)?;

        let fps = decoder.get_fps();
        let width = decoder.width();
        let height = decoder.height();
        let duration = decoder.duration();
        let delay_ms = if fps > 0.0 { (1000.0 / fps) as u64 } else { 33 };

        let thread_handle = thread::spawn(move || {
            let mut is_paused = true;

            loop {
                // Check for control commands from the UI thread.
                match control_receiver.try_recv() {
                    Ok(ControlCommand::Play) => is_paused = false,
                    Ok(ControlCommand::Pause) => is_paused = true,
                    Ok(ControlCommand::Seek(ms)) => {
                        if decoder.cap.set(videoio::CAP_PROP_POS_MSEC, ms).is_err() {
                            eprintln!("Seek failed to position {}ms", ms);
                        }
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        // UI thread has disconnected, terminate.
                        break;
                    }
                    Err(mpsc::TryRecvError::Empty) => { /* No command */ }
                }

                if !is_paused {
                    match decoder.read_next_frame() {
                        Ok(Some((color_image, timestamp_ms))) => {
                            // Send the converted image to the UI thread.
                            if image_sender.send(Ok((color_image, timestamp_ms))).is_err() {
                                // If sending fails, terminate the thread.
                                break;
                            }
                        }
                        Ok(None) => {
                            // End of video.
                            break;
                        }
                        Err(err) => {
                            // Send the error and terminate the thread.
                            let _ = image_sender.send(Err(err));
                            break;
                        }
                    }
                }

                // Sleep for the calculated delay to reduce CPU usage, even when paused.
                thread::sleep(std::time::Duration::from_millis(delay_ms));
            }
        });

        Ok(Self {
            _thread_handle: thread_handle,
            width,
            height,
            duration,
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn duration(&self) -> f64 {
        self.duration
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Write, path::PathBuf};
    use tempfile::NamedTempFile;

    /// Helper to create a temporary empty file.
    fn create_empty_temp_file() -> PathBuf {
        let mut file = NamedTempFile::new().expect("Failed to create temporary file");
        file.write_all(b"")
            .expect("Failed to write to temporary file");
        file.path().to_path_buf()
    }

    #[test]
    fn test_frame_decoder_new_non_existent_file() {
        let non_existent_path = PathBuf::from("non_existent_video_file.mp4");
        let decoder_result = FrameDecoder::new(&non_existent_path);
        assert!(decoder_result.is_err());
        if let Err(VideoReaderError::OpenCV(msg)) = decoder_result {
            assert!(msg.contains("Failed to open video file"));
        } else {
            panic!("Expected an OpenCV error for non-existent file.");
        }
    }

    #[test]
    fn test_frame_decoder_new_empty_file() {
        let empty_file_path = create_empty_temp_file();
        let decoder_result = FrameDecoder::new(&empty_file_path);
        // Expecting an error because an empty file is not a valid video
        assert!(
            decoder_result.is_err(),
            "FrameDecoder::new should return an error for an empty file."
        );
        if let Err(VideoReaderError::OpenCV(msg)) = decoder_result {
            assert!(msg.contains("Failed to open video file"));
        } else {
            panic!("Expected an OpenCV error for empty file.");
        }
    }

    #[test]
    fn test_frame_decoder_read_next_frame_empty_file() {
        let empty_file_path = create_empty_temp_file();
        let decoder_result = FrameDecoder::new(&empty_file_path);
        // Expecting an error because an empty file is not a valid video
        assert!(
            decoder_result.is_err(),
            "FrameDecoder::new should return an error for an empty file before attempting to read frames."
        );
        if let Err(VideoReaderError::OpenCV(msg)) = decoder_result {
            assert!(msg.contains("Failed to open video file"));
        } else {
            panic!("Expected an OpenCV error when creating FrameDecoder for empty file.");
        }
    }
}
