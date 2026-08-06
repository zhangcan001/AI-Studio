use image::{ImageFormat, ImageReader};
use sha2::{Digest, Sha256};
use std::{error::Error, fmt, io::Cursor};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct InspectedImage {
    pub(crate) extension: &'static str,
    pub(crate) mime_type: &'static str,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ImageInspectionError {
    message: String,
}

impl ImageInspectionError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for ImageInspectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for ImageInspectionError {}

pub(crate) fn inspect_bytes(bytes: &[u8]) -> Result<InspectedImage, ImageInspectionError> {
    if bytes.is_empty() {
        return Err(ImageInspectionError::new("image is empty"));
    }

    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|error| ImageInspectionError::new(error.to_string()))?;
    let format = reader
        .format()
        .ok_or_else(|| ImageInspectionError::new("image format could not be detected"))?;
    let (width, height) = reader
        .into_dimensions()
        .map_err(|error| ImageInspectionError::new(error.to_string()))?;
    let (extension, mime_type) = match format {
        ImageFormat::Png => ("png", "image/png"),
        ImageFormat::Jpeg => ("jpg", "image/jpeg"),
        ImageFormat::WebP => ("webp", "image/webp"),
        other => {
            return Err(ImageInspectionError::new(format!(
                "unsupported image format {other:?}"
            )))
        }
    };

    Ok(InspectedImage {
        extension,
        mime_type,
        width,
        height,
        sha256: format!("{:x}", Sha256::digest(bytes)),
    })
}

pub(crate) fn generate_thumbnail(bytes: &[u8]) -> Result<Vec<u8>, ImageInspectionError> {
    let image = image::load_from_memory(bytes)
        .map_err(|error| ImageInspectionError::new(format!("thumbnail decode failed: {error}")))?;
    let thumbnail = image.thumbnail(384, 384);
    let mut output = Cursor::new(Vec::new());
    thumbnail
        .write_to(&mut output, ImageFormat::Png)
        .map_err(|error| ImageInspectionError::new(format!("thumbnail encode failed: {error}")))?;
    Ok(output.into_inner())
}
