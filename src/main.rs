use eframe::egui;
use opencv::prelude::*;
use opencv::{core, imgproc, videoio};
use std::env;
use std::sync::{Arc, Mutex};

fn main() -> eframe::Result<()> {
    let args: Vec<String> = env::args().collect();
    let initial_path = args.get(1).cloned();

    // application option config
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([800.0, 600.0]), // default window size
        ..Default::default()
    };

    // start app
    eframe::run_native(
        "Object detection evaluator",
        options,
        Box::new(|_cc| Ok(Box::new(MyApp::default()))),
    )
}

struct MyApp {
    // video capture
    // put into Arc<Mutex> to share between threads
    caputure: Option<Arc<Mutex<videoio::VideoCapture>>>,
    // texture to display on the screen
    texture: Option<egui::TextureHandle>,
    // error message
    error_msg: Option<String>,
}

impl Default for MyApp {
    fn default() -> Self {
        Self {
            caputure: None,
            texture: None,
            error_msg: None,
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Object Detection Evaluator");

            ui.horizontal(|ui| {
                // file select button
                if ui.button("Select Video File...").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Video", &["mp4", "avi", "mov"])
                        .pick_file()
                    {
                        // open a video with opencv
                        // path to string
                        let path_str = path.to_str().unwrap_or_default();

                        match videoio::VideoCapture::from_file(path_str, videoio::CAP_ANY) {
                            Ok(cap) => {
                                self.caputure = Some(Arc::new(Mutex::new(cap)));
                                self.error_msg = None;
                            }
                            Err(e) => {
                                self.error_msg = Some(format!("Failed to open video: {:?}", e));
                            }
                        }
                    }
                }
            });

            // display an error
            if let Some(msg) = &self.error_msg {
                ui.colored_label(egui::Color32::RED, msg);
            }

            // video playback processing
            if let Some(cap_arc) = &self.caputure {
                // read frame (lock mutext)
                // originally, this should be read in a separate thread,
                // but for a simplified implementation, we'll read it here.
                if let Ok(mut cap) = cap_arc.lock() {
                    let mut frame = core::Mat::default();

                    // get a frame
                    if cap.read(&mut frame).unwrap_or(false) && !frame.empty() {
                        // convert OpenCV(BGR) -> egui(RGB)
                        let mut rgb_frame = core::Mat::default();
                        imgproc::cvt_color(&frame, &mut rgb_frame, imgproc::COLOR_BGR2RGB, 0).ok();

                        // get Mat data as a byte array
                        let w = rgb_frame.cols();
                        let h = rgb_frame.rows();

                        let data = unsafe {
                            std::slice::from_raw_parts(
                                rgb_frame.data() as *const u8,
                                (w * h * 3) as usize,
                            )
                        };

                        // create image data for egui
                        let color_image =
                            egui::ColorImage::from_rgb([w as usize, h as usize], data);

                        // update texture or create
                        self.texture = Some(ctx.load_texture(
                            "video_frame",
                            color_image,
                            egui::TextureOptions::LINEAR,
                        ));

                        // request UI update to immediately render the next frame
                        ctx.request_repaint();
                    } else {
                        ui.label("再生終了");
                    }
                }
            }

            // render to display
            if let Some(texture) = &self.texture {
                // Shrink to fit window size
                ui.image((texture.id(), texture.size_vec2()));
            } else {
                ui.label("Please load a video file");
            }
        });
    }
}
