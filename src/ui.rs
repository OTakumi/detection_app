use crate::video_reader::{VideoReader, VideoReaderError};
use eframe::egui;
use std::sync::mpsc;

// Enum to manage the playback state.
enum PlaybackState {
    // Initial state where no video is loaded.
    NotLoaded,
    // Video is currently playing.
    Playing,
    // Playback has finished.
    Finished,
    // An error has occurred.
    Error(String),
}

pub struct MyApp {
    // Manages the video reading thread.
    video_reader: Option<VideoReader>,
    // Receiver for video frames.
    image_receiver: mpsc::Receiver<Result<egui::ColorImage, VideoReaderError>>,
    // Sender to be passed to the VideoReader.
    image_sender: mpsc::Sender<Result<egui::ColorImage, VideoReaderError>>,
    // Texture to display on the screen.
    texture: Option<egui::TextureHandle>,
    // The current playback state.
    playback_state: PlaybackState,
}

impl Default for MyApp {
    fn default() -> Self {
        // Create a communication channel between the UI thread and the video processing thread.
        let (image_sender, image_receiver) = mpsc::channel();
        Self {
            video_reader: None,
            image_receiver,
            image_sender,
            texture: None,
            playback_state: PlaybackState::NotLoaded,
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Check for a new frame from the background thread.
        match self.image_receiver.try_recv() {
            // Received new image data.
            Ok(Ok(color_image)) => {
                self.playback_state = PlaybackState::Playing;
                // Update the texture with the received image data.
                self.texture = Some(ctx.load_texture(
                    "video_frame",
                    color_image,
                    egui::TextureOptions::LINEAR,
                ));
            }
            // An error occurred during video processing.
            Ok(Err(VideoReaderError::OpenCV(msg))) => {
                self.playback_state =
                    PlaybackState::Error(format!("Video processing error: {}", msg));
                self.video_reader = None; // Stop the reader because an error occurred.
            }
            // Channel disconnected (video playback finished).
            Err(mpsc::TryRecvError::Disconnected) => {
                // If playback was in progress, update the state to "Finished".
                if matches!(self.playback_state, PlaybackState::Playing) {
                    self.playback_state = PlaybackState::Finished;
                }
                self.video_reader = None; // Discard the reader.
            }
            // No new data has arrived yet.
            Err(mpsc::TryRecvError::Empty) => {}
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Object Detection Evaluator");

            ui.horizontal(|ui| {
                // File select button
                // Create a video reader when a video is selected.
                if ui.button("Select Video File...").clicked()
                    && let Some(path) = rfd::FileDialog::new()
                        .add_filter("Video", &["mp4", "avi", "mov"])
                        .pick_file()
                {
                    // Discard existing video reader and clear texture
                    self.video_reader = None;
                    self.texture = None;

                    // Create a new video reader, but reuse the channel
                    match VideoReader::new(&path, self.image_sender.clone()) {
                        Ok(reader) => {
                            self.video_reader = Some(reader);
                            self.playback_state = PlaybackState::Playing;
                        }
                        Err(VideoReaderError::OpenCV(msg)) => {
                            self.playback_state =
                                PlaybackState::Error(format!("Failed to open video: {}", msg));
                        }
                    }
                }
            });

            ui.separator();

            // Update the UI based on the current playback state.
            match &self.playback_state {
                PlaybackState::Error(msg) => {
                    ui.colored_label(egui::Color32::RED, msg);
                }
                PlaybackState::NotLoaded => {
                    ui.label("Please load a video file.");
                }
                PlaybackState::Finished => {
                    if let Some(texture) = &self.texture {
                        ui.image((texture.id(), texture.size_vec2()));
                    }
                    ui.label("Playback finished.");
                }
                PlaybackState::Playing => {
                    if let Some(texture) = &self.texture {
                        ui.image((texture.id(), texture.size_vec2()));
                    } else {
                        // Display while waiting for the first frame.
                        ui.label("Loading video...");
                    }
                }
            }
        });

        // Constantly request UI redraws to keep the animation smooth.
        ctx.request_repaint();
    }
}
