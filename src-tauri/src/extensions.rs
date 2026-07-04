//! Single place to edit the list of video file extensions the importer
//! recognizes. Extend this if a new camera/drone shows up with a container
//! format that isn't covered.

pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    "mp4", "mov", "m4v", "avi", // common cameras/phones
    "mts", "m2ts", // AVCHD camcorders
    "3gp", "wmv", "mkv", "mpg", "mpeg", // older/misc cameras
    "insv", // Insta360
];

pub fn get_supported_extensions() -> Vec<String> {
    SUPPORTED_EXTENSIONS.iter().map(|s| s.to_string()).collect()
}

pub fn is_supported_extension(ext: &str) -> bool {
    let lower = ext.to_ascii_lowercase();
    SUPPORTED_EXTENSIONS.contains(&lower.as_str())
}
