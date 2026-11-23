use image::{
    DynamicImage, ImageBuffer, Rgb,
    imageops::{self, FilterType},
};

use crate::utils::{LastFrameType, MODEL_RESOLUTION};

pub fn prepare_frame(last_frame: LastFrameType) -> Option<ImageBuffer<Rgb<u8>, Vec<u8>>> {
    if let Some((frame, resolution)) = last_frame {
        let mut buffer_image = ImageBuffer::<Rgb<u8>, Vec<u8>>::from_pixel(
            MODEL_RESOLUTION.0,
            MODEL_RESOLUTION.1,
            Rgb([114, 114, 114]),
        );

        let mut frame_image = DynamicImage::ImageRgba8(frame).into_rgb8();
        if let Some(res) = resolution {
            // fast_resize_image crate
            frame_image = imageops::resize(
                &frame_image,
                res.width_x,
                res.height_y,
                FilterType::Triangle,
            );
        }

        imageops::overlay(
            &mut buffer_image,
            &frame_image,
            0,
            (MODEL_RESOLUTION.1 / 2 - (frame_image.height() / 2)) as i64,
        );

        return Some(buffer_image);
    }

    None
}
