use anyhow::{anyhow, bail};
use gpui_kit::RenderImage;
use image::Frame;
use nokhwa::{
    Camera, NokhwaError,
    pixel_format::{RgbAFormat, RgbFormat},
    utils::{
        ApiBackend, CameraFormat, CameraIndex, CameraInfo, FrameFormat, RequestedFormat,
        RequestedFormatType::{self, AbsoluteHighestFrameRate},
        Resolution,
    },
};
use thiserror::Error;
use tokio::sync::watch::Sender;

use crate::utils::{LastFrameType, MODEL_RESOLUTION, calculate_aspect_ratio_from};

pub struct Capture {
    pub device_listage: Vec<CameraInfo>,
}

#[derive(Error, Debug)]
pub enum GetFrameError {
    #[error("Something goes wrong on request camera frame: {0}")]
    FailedToGetFrame(String),
    #[error("Capture engine error: {0}")]
    CaptureEngineError(#[from] NokhwaError),
    #[error("Failed to send the rgba frame in channel: {0}")]
    SendLastFrame(#[from] tokio::sync::watch::error::SendError<LastFrameType>),
}

impl Capture {
    pub fn new() -> anyhow::Result<Self> {
        let supported_devices = nokhwa::query(ApiBackend::Auto)
            .map_err(|e| anyhow!("Failed to list supported devices: {e}"))?;

        if supported_devices.is_empty() {
            bail!("Default camera not found");
        }

        Ok(Self {
            device_listage: supported_devices,
        })
    }

    pub fn set_camera(&self, cam_index: usize) -> anyhow::Result<(String, CameraIndex)> {
        if cam_index >= self.device_listage.len() {
            bail!("Wrong index selected");
        }

        let device = self.device_listage[cam_index].clone();

        let device_name = device.human_name();
        let device_index = device.index();

        Ok((device_name, device_index.to_owned()))
    }

    pub fn process_frame(
        &self,
        camera_instance: &mut Camera,
        last_frame_sender: &Sender<LastFrameType>,
        cam_resolution: Option<Resolution>,
    ) -> Result<RenderImage, GetFrameError> {
        let frame = camera_instance
            .frame()
            .map_err(|e| GetFrameError::FailedToGetFrame(e.to_string()))?;

        let mut image = frame.decode_image::<RgbAFormat>()?;
        image::imageops::flip_horizontal_in_place(&mut image);

        last_frame_sender.send(Some((image.clone(), cam_resolution)))?;

        for chunk in image.chunks_exact_mut(4) {
            chunk.swap(0, 2); // convert from rgba to bgra
        }

        let to_frame = Frame::new(image);
        let render = RenderImage::new(vec![to_frame]);

        Ok(render)
    }

    pub fn camera_handle(
        device_index: CameraIndex,
    ) -> Result<(Camera, Option<Resolution>), anyhow::Error> {
        let mut cam_resolution: Option<Resolution> = None;
        let mut instance = Camera::new(
            device_index.to_owned(),
            RequestedFormat::new::<RgbFormat>(AbsoluteHighestFrameRate),
        )
        .map_err(|e| anyhow!(e))?;

        let device_resolution = instance.resolution();
        let target_aspect_ratio = calculate_aspect_ratio_from(
            (device_resolution.width_x, device_resolution.height_y),
            MODEL_RESOLUTION.0,
        );

        let mut formats = instance
            .compatible_camera_formats()
            .map_err(|e| anyhow!(e))?;

        formats.sort();
        formats.dedup();

        let available: Option<CameraFormat> = formats
            .into_iter()
            .filter(|f| {
                let res = f.resolution();
                res.width_x == target_aspect_ratio.0
                    && res.height_y == target_aspect_ratio.1
                    && f.format() == FrameFormat::NV12
            })
            .max_by_key(|f| f.frame_rate());

        let camera = match available {
            Some(cam_format) => Camera::new(
                device_index.to_owned(),
                RequestedFormat::new::<RgbFormat>(RequestedFormatType::Exact(cam_format)),
            )
            .map_err(|e| anyhow!(e))?,
            None => {
                cam_resolution = Some(Resolution {
                    width_x: target_aspect_ratio.0,
                    height_y: target_aspect_ratio.1,
                });
                instance
            }
        };

        Ok((camera, cam_resolution))
    }
}
