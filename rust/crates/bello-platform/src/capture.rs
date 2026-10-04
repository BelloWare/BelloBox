use crate::{backend, ErrorKind, PlatformError, Result};
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_SCREENSHOT_BYTES: u64 = 100 * 1024 * 1024;
static NEXT_STAGING_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedScreenshot {
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub byte_len: u64,
    pub backend: &'static str,
}

pub(crate) fn capture(destination: &Path) -> Result<CapturedScreenshot> {
    if !has_png_extension(destination) || destination.file_name().is_none() {
        return Err(error(
            ErrorKind::InvalidInput,
            "Choose a new destination filename ending in .png.",
        ));
    }
    let parent = destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let parent = parent
        .canonicalize()
        .map_err(|_| error(ErrorKind::Io, "The destination folder could not be opened."))?;
    if !parent.is_dir() {
        return Err(error(
            ErrorKind::InvalidInput,
            "Choose an existing destination folder.",
        ));
    }
    let destination = parent.join(destination.file_name().expect("validated filename"));
    // symlink_metadata also rejects dangling symlinks, before any capture occurs.
    match destination.symlink_metadata() {
        Ok(_) => {
            return Err(error(
                ErrorKind::AlreadyExists,
                "The destination already exists; choose a new filename.",
            ))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => {
            return Err(error(
                ErrorKind::Io,
                "The destination could not be inspected.",
            ))
        }
    }
    let staging = Staging::create(&parent)?;
    let backend = backend::capture_to(&staging.image)?;
    let (width, height, byte_len) = validate_png(&staging.image)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&staging.image, fs::Permissions::from_mode(0o600)).map_err(|_| {
            error(
                ErrorKind::Io,
                "Could not restrict screenshot file permissions.",
            )
        })?;
    }
    // Hard-link publication is atomic and cannot replace a file created after
    // the existence check above. Staging is on the same filesystem as the target.
    fs::hard_link(&staging.image, &destination).map_err(|e| {
        if e.kind() == std::io::ErrorKind::AlreadyExists { error(ErrorKind::AlreadyExists, "The destination now exists; no file was replaced.") }
        else { error(ErrorKind::Io, "Could not publish the screenshot; the destination filesystem must allow same-folder hard links.") }
    })?;
    Ok(CapturedScreenshot {
        path: destination,
        width,
        height,
        byte_len,
        backend,
    })
}

fn has_png_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.eq_ignore_ascii_case("png"))
}

struct Staging {
    directory: PathBuf,
    image: PathBuf,
}

impl Staging {
    fn create(parent: &Path) -> Result<Self> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        for _ in 0..32 {
            let sequence = NEXT_STAGING_ID.fetch_add(1, Ordering::Relaxed);
            let directory = parent.join(format!(
                ".bellobox-capture-{}-{timestamp}-{sequence}",
                std::process::id()
            ));
            let mut builder = fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            match builder.create(&directory) {
                Ok(()) => {
                    return Ok(Self {
                        image: directory.join("capture.png"),
                        directory,
                    })
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(_) => {
                    return Err(error(
                        ErrorKind::Io,
                        "Could not create a private staging folder in the destination folder.",
                    ))
                }
            }
        }
        Err(error(
            ErrorKind::Io,
            "Could not allocate a unique screenshot staging folder.",
        ))
    }
}

impl Drop for Staging {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.image);
        let _ = fs::remove_dir(&self.directory);
    }
}

fn validate_png(path: &Path) -> Result<(u32, u32, u64)> {
    let mut file = fs::File::open(path).map_err(|_| {
        error(
            ErrorKind::InvalidOutput,
            "The screenshot helper did not create an image.",
        )
    })?;
    let metadata = file
        .metadata()
        .map_err(|_| error(ErrorKind::Io, "Could not inspect the captured image."))?;
    if !metadata.is_file() || metadata.len() < 45 {
        return Err(error(
            ErrorKind::InvalidOutput,
            "The screenshot helper created an incomplete image.",
        ));
    }
    if metadata.len() > MAX_SCREENSHOT_BYTES {
        return Err(error(
            ErrorKind::OutputTooLarge,
            "Screenshot exceeds the 100 MiB file limit.",
        ));
    }
    let mut header = [0; 24];
    file.read_exact(&mut header).map_err(|_| {
        error(
            ErrorKind::InvalidOutput,
            "Could not read the screenshot PNG header.",
        )
    })?;
    let (width, height) = png_dimensions(&header)?;
    file.seek(SeekFrom::End(-12)).map_err(|_| {
        error(
            ErrorKind::InvalidOutput,
            "Could not inspect the screenshot PNG ending.",
        )
    })?;
    let mut ending = [0; 12];
    file.read_exact(&mut ending).map_err(|_| {
        error(
            ErrorKind::InvalidOutput,
            "The screenshot PNG is incomplete.",
        )
    })?;
    if ending != *b"\0\0\0\0IEND\xaeB`\x82" {
        return Err(error(
            ErrorKind::InvalidOutput,
            "The screenshot PNG has no valid ending; no destination file was created.",
        ));
    }
    Ok((width, height, metadata.len()))
}

fn png_dimensions(header: &[u8]) -> Result<(u32, u32)> {
    if header.len() < 24 || &header[..16] != b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR" {
        return Err(error(
            ErrorKind::InvalidOutput,
            "The screenshot helper did not produce a PNG image.",
        ));
    }
    let width = u32::from_be_bytes(header[16..20].try_into().expect("four bytes"));
    let height = u32::from_be_bytes(header[20..24].try_into().expect("four bytes"));
    if width == 0
        || height == 0
        || width > 32768
        || height > 32768
        || u64::from(width) * u64::from(height) > 200_000_000
    {
        return Err(error(
            ErrorKind::OutputTooLarge,
            "Screenshot dimensions are invalid or exceed the 200-megapixel/32768-pixel limits.",
        ));
    }
    Ok((width, height))
}

fn error(kind: ErrorKind, message: &'static str) -> PlatformError {
    PlatformError::new(kind, "Capture screenshot", message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(width: u32, height: u32) -> Vec<u8> {
        let mut h = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        h.extend(width.to_be_bytes());
        h.extend(height.to_be_bytes());
        h
    }

    #[test]
    fn png_header_dimensions_are_bounded_without_integer_overflow() {
        assert_eq!(png_dimensions(&header(1920, 1080)).unwrap(), (1920, 1080));
        for (width, height) in [
            (0, 1),
            (1, 0),
            (32769, 1),
            (1, 32769),
            (u32::MAX, u32::MAX),
            (32768, 32768),
        ] {
            assert!(png_dimensions(&header(width, height)).is_err());
        }
        assert!(png_dimensions(b"not a PNG").is_err());
        let mut wrong = header(1, 1);
        wrong[15] = b'X';
        assert!(png_dimensions(&wrong).is_err());
    }

    #[test]
    fn capture_filename_is_explicit_png() {
        assert!(has_png_extension(Path::new("shot.PNG")));
        assert!(has_png_extension(Path::new("/tmp/a folder/shot.png")));
        assert!(!has_png_extension(Path::new("shot.jpg")));
        assert!(!has_png_extension(Path::new(".png")));
    }
}
