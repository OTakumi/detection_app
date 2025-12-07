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

pub struct VideoReader {
    // Holds the handle of the spawned thread.
    // It's possible to implement `drop` to join the thread when the VideoReader is dropped,
    // but for this implementation, the thread is designed to terminate autonomously when the UI closes and the channel is disconnected,
    // so the handle is not used directly.
    _thread_handle: thread::JoinHandle<()>,
}

impl VideoReader {
    /// Creates a new VideoReader and starts reading the video on a background thread.
    ///
    /// # Arguments
    /// * `path` - The path to the video file to read.
    /// * `image_sender` - The sender to send the read frames (`egui::ColorImage`) to the UI thread.
    ///
    /// # Returns
    /// * `Ok(Self)` - If the thread was successfully started.
    /// * `Err(VideoReaderError)` - If opening the video file fails.
    pub fn new(
        path: &Path,
        image_sender: mpsc::Sender<Result<egui::ColorImage, VideoReaderError>>,
    ) -> Result<Self, VideoReaderError> {
        let path_str = path.to_str().unwrap_or_default().to_string();
        let mut cap = VideoCapture::from_file(&path_str, videoio::CAP_ANY)
            .map_err(|e| VideoReaderError::OpenCV(format!("Failed to open video file: {:?}", e)))?;

        // Get the frame rate to calculate the delay between frames.
        let fps = cap.get(videoio::CAP_PROP_FPS).unwrap_or(30.0);
        let delay_ms = if fps > 0.0 { (1000.0 / fps) as u64 } else { 33 }; // Avoid division by zero.

        let thread_handle = thread::spawn(move || {
            loop {
                let mut frame = core::Mat::default();

                match cap.read(&mut frame) {
                    // If a frame is read successfully and is not empty
                    Ok(true) if !frame.empty() => {
                        // Convert from OpenCV's BGR format to egui's RGB format.
                        let mut rgb_frame = core::Mat::default();

                        if imgproc::cvt_color(&frame, &mut rgb_frame, imgproc::COLOR_BGR2RGB, 0)
                            .is_err()
                        {
                            let err = VideoReaderError::OpenCV(
                                "Failed to convert frame to RGB".to_string(),
                            );
                            // Send the error and terminate the thread.
                            let _ = image_sender.send(Err(err));
                            break;
                        }

                        // Convert Mat data to egui::ColorImage.
                        // This operation is unsafe, but it is safe because we have confirmed that the lifetime of the Mat and the size of the data are correct.
                        let size = rgb_frame.size().expect("Failed to get frame size");
                        let data = unsafe {
                            std::slice::from_raw_parts(
                                rgb_frame.data(),
                                size.width as usize * size.height as usize * 3,
                            )
                        };

                        // `from_rgb` copies the data internally, so releasing `rgb_frame` afterwards is not a problem.
                        let color_image = egui::ColorImage::from_rgb(
                            [size.width as usize, size.height as usize],
                            data,
                        );

                        // Send the converted image to the UI thread.
                        if image_sender.send(Ok(color_image)).is_err() {
                            // If sending fails (e.g., the UI has dropped the receiver),
                            // terminate the thread.
                            break;
                        }
                    }
                    // If the end of the video is reached or a read error occurs
                    _ => {
                        break;
                    }
                }
                // Sleep for the calculated delay to reduce CPU usage.
                thread::sleep(std::time::Duration::from_millis(delay_ms));
            }
        });

        Ok(Self {
            _thread_handle: thread_handle,
        })
    }
}
