/// Enum representing control commands for the video reader.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ControlCommand {
    Play,
    Pause,
    Seek(f64),
}
