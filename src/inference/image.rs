use anyhow::{anyhow, bail};
use image::{
    DynamicImage, ImageBuffer, Rgb,
    imageops::{self, FilterType},
};
use ort::{ep, inputs, session::Session, value::TensorRef};
use tokio::sync::watch::Receiver;
use tokio_util::sync::CancellationToken;

use crate::{inference::{BoundingBox, InferOutput, KeyPoint}, utils::{LastFrameType, MODEL_RESOLUTION}};

pub struct InferLayer {
    cancel_token: CancellationToken,
    last_frame_channel: Receiver<LastFrameType>,
    session: Session,
}

impl InferLayer {
    pub fn new(token: CancellationToken, channel: Receiver<LastFrameType>) -> anyhow::Result<Self> {
        let env = ort::init().build()?;
        let session = Session::builder(&env)?
            .with_execution_providers([ep::CUDA::default().build()])
            .map_err(|e| anyhow!(e.to_string()))?
            .commit_from_file("models/yolo26_hand_pose_fp32.onnx")?;

        Ok(Self {
            session,
            cancel_token: token,
            last_frame_channel: channel,
        })
    }

    pub fn prepare_frame(
        &self,
        last_frame: LastFrameType,
    ) -> Option<ImageBuffer<Rgb<u8>, Vec<u8>>> {
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

    pub fn run_engine(&mut self) -> anyhow::Result<()> {
        loop {
            if self.cancel_token.is_cancelled() {
                break;
            }

            match self.last_frame_channel.has_changed() {
                Ok(status) => {
                    if !status {
                        continue;
                    }

                    let last_frame = self.last_frame_channel.borrow_and_update().clone();
                    let prepared_frame = self.prepare_frame(last_frame);

                    let mut input = ndarray::Array4::<f32>::zeros((1, 3, 640, 640));

                    if let Some(frame) = prepared_frame {
                        for (x, y, pixel) in frame.enumerate_pixels() {
                            let [r, g, b] = pixel.0;

                            input[[0, 0, y as _, x as _]] = (r as f32) / 255.;
                            input[[0, 1, y as _, x as _]] = (g as f32) / 255.;
                            input[[0, 2, y as _, x as _]] = (b as f32) / 255.;
                        }

                        let tensor = TensorRef::from_array_view(&input)?;
                        let outputs = self.session.run(inputs!["images" => tensor])?;
                        let output = outputs["output0"].try_extract_array::<f32>()?.into_owned();

                        let rows = output.shape()[1];
                        let col_size = output.shape()[2];

                        let mut output_payload: Vec<InferOutput> = Vec::new();

                        for line in 0..rows {
                            let score = output[[0, line, 4]];

                            if score < 0.5 {
                                continue;
                            }
                            
                            let mut infer_output = InferOutput::default();

                            infer_output.bounding_box = BoundingBox {
                                x1: output[[0, line, 0]],
                                y1: output[[0, line, 1]],
                                x2: output[[0, line, 2]],
                                y2: output[[0, line, 3]],
                            };

                            infer_output.score = score;

                            let mut temp = Vec::new();
                            for key in 6..col_size {
                                temp.push(output[[0, line, key]]);
                            }

                            for data in temp.chunks_exact(3) {
                                infer_output.key_points.push(KeyPoint {
                                    x: data[0],
                                    y: data[1],
                                    visibility: data[2],
                                });
                            }

                            output_payload.push(infer_output);
                        }

                        println!("{:?}", output_payload);
                    }
                }
                Err(e) => {
                    self.cancel_token.cancel();
                    bail!("Receiver last frame channel error: {e}");
                }
            }
        }

        Ok(())
    }
}
