use anyhow::{anyhow, bail};
use gpui_kit::RenderImage;
use image::Frame;
use nokhwa::{
    Camera, NokhwaError,
    pixel_format::RgbAFormat,
    utils::{ApiBackend, CameraIndex, CameraInfo, Resolution},
};
use thiserror::Error;
use tokio::sync::watch::Sender;

use crate::utils::LastFrameType;

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

        if supported_devices.len() == 0 {
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

        let device = self.device_listage[cam_index as usize].clone();

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

        last_frame_sender.send(Some((image.clone(), cam_resolution.clone())))?;

        for chunk in image.chunks_exact_mut(4) {
            chunk.swap(0, 2); // convert from rgba to bgra
        }

        let to_frame = Frame::new(image);
        let render = RenderImage::new(vec![to_frame]);

        Ok(render)
    }
}
