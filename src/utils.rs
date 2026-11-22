use image::{ImageBuffer, Rgba};
use nokhwa::utils::Resolution;

pub type LastFrameType = Option<(ImageBuffer<Rgba<u8>, Vec<u8>>, Option<Resolution>)>;

pub fn calculate_aspect_ratio_from(input: (u32, u32), tw: u32) -> (u32, u32) {
    return (tw, tw * input.1 / input.0);
}
