use std::{sync::Arc, time::Instant};

use anyhow::{anyhow, bail};
use image::{
    DynamicImage, ImageBuffer, Rgb,
    imageops::{self, FilterType},
};
use nokhwa::{
    Camera,
    pixel_format::RgbFormat,
    utils::{
        CameraFormat, FrameFormat, RequestedFormat,
        RequestedFormatType::{self, AbsoluteHighestFrameRate},
        Resolution,
    },
};
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;
use tracing::{Level, error};
use tracing_subscriber::FmtSubscriber;

use gpui_kit::*;

use crate::{
    capture::Capture,
    ui::MainWindow,
    utils::{LastFrameType, calculate_aspect_ratio_from},
};

mod capture;
mod ui;
mod utils;

const MODEL_RESOLUTION: (u32, u32) = (640, 640);

fn main() -> anyhow::Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let application_token = CancellationToken::new();

    let capture_instance = Capture::new()?;
    let (device_name, device_index) = capture_instance.set_camera(0)?;

    let (sender_image, receiver_image) = watch::channel::<Option<Arc<RenderImage>>>(None);
    let (last_frame_sender, mut last_frame_receiver) = watch::channel::<LastFrameType>(None);

    let capture_token = application_token.clone();

    std::thread::spawn(move || {
        let mut cam_resolution: Option<Resolution> = None;
        let mut camera = match Camera::new(
            device_index.to_owned(),
            RequestedFormat::new::<RgbFormat>(AbsoluteHighestFrameRate),
        ) {
            Ok(mut instance) => {
                let device_resolution = instance.resolution();
                let target_aspect_ratio = calculate_aspect_ratio_from(
                    (device_resolution.width_x, device_resolution.height_y),
                    MODEL_RESOLUTION.0,
                );

                let mut formats = instance
                    .compatible_camera_formats()
                    .map_err(|e| format!("Not was possible list camera format: {e}"))
                    .unwrap();

                formats.sort();
                formats.dedup();

                let available: Option<CameraFormat> = formats
                    .into_iter()
                    .filter(|f| {
                        let res = f.resolution();
                        return res.width_x == target_aspect_ratio.0
                            && res.height_y == target_aspect_ratio.1
                            && f.format() == FrameFormat::NV12;
                    })
                    .max_by_key(|f| f.frame_rate());

                match available {
                    Some(cam_format) => Camera::new(
                        device_index.to_owned(),
                        RequestedFormat::new::<RgbFormat>(RequestedFormatType::Exact(cam_format)),
                    )
                    .unwrap(),
                    None => {
                        cam_resolution = Some(Resolution {
                            width_x: target_aspect_ratio.0,
                            height_y: target_aspect_ratio.1,
                        });
                        instance
                    }
                }
            }
            Err(e) => {
                capture_token.cancel();
                bail!("Failed to get camera instance: {e}");
            }
        };

        loop {
            if capture_token.is_cancelled() {
                break Ok(());
            }

            match capture_instance.process_frame(&mut camera, &last_frame_sender, cam_resolution) {
                Ok(frame) => {
                    sender_image
                        .send(Some(Arc::new(frame)))
                        .expect("Failed to send image to UI");
                }
                Err(e) => match e {
                    _ => {
                        capture_token.cancel();
                        error!("{e}");
                        break Ok(());
                    }
                },
            }
        }
    });

    let infer_token = application_token.clone();

    std::thread::spawn(move || {
        let mut saved = false;

        loop {
            if infer_token.is_cancelled() {
                break;
            }

            match last_frame_receiver.has_changed() {
                Ok(status) => {
                    if status == false {
                        continue;
                    }

                    let current_frame = last_frame_receiver.borrow_and_update().clone();

                    //                     if saved == false {
                    //                         if let Some((frame, resolution)) = current_frame {
                    //                             let t = Instant::now();
                    //                             let mut buffer_image = ImageBuffer::<Rgb<u8>, Vec<u8>>::from_pixel(
                    //                                 MODEL_RESOLUTION.0,
                    //                                 MODEL_RESOLUTION.1,
                    //                                 Rgb([114, 114, 114]),
                    //                             );
                    //
                    //                             let resized_frame = imageops::resize(
                    //                                 &frame,
                    //                                 resolution.width_x,
                    //                                 resolution.height_y,
                    //                                 FilterType::Triangle,
                    //                             );
                    //
                    //                             let frame_rgb = DynamicImage::ImageRgba8(resized_frame).into_rgb8();
                    //                             imageops::overlay(
                    //                                 &mut buffer_image,
                    //                                 &frame_rgb,
                    //                                 0,
                    //                                 (MODEL_RESOLUTION.1 / 2 - (resolution.height_y / 2)) as i64,
                    //                             );
                    //
                    //                             let _ = buffer_image.save("img.png");
                    //                             println!("The process takes: {:?}", t.elapsed());
                    //                             saved = true;
                    //                         }
                    //                     }
                }
                Err(e) => {
                    infer_token.cancel();
                    error!("Receiver last frame channel error: {e}");
                    break;
                }
            }
        }
    });

    let ui_token = application_token.clone();

    application().with_assets(assets::Assets).run(|cx| {
        init(cx);

        cx.spawn(async move |app| {
            while ui_token.is_cancelled() {
                app.update(|app_ctx| {
                    app_ctx.quit();
                });
                break;
            }
        })
        .detach();

        let mut default_position = Point {
            x: px(10.0),
            y: px(10.0),
        };
        let window_size = Size {
            width: px(495.0),
            height: px(335.0),
        };

        if let Some(display) = cx.primary_display() {
            let display_bounds = display.bounds().size;

            default_position.x =
                px(display_bounds.width.as_f32() / 2. - (window_size.width.as_f32() / 2.));
            default_position.y =
                px(display_bounds.height.as_f32() / 2. - (window_size.height.as_f32() / 2.));
        }

        open_window(
            WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some("Motion track control app".into()),
                    appears_transparent: false,
                    ..Default::default()
                }),
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: default_position,
                    size: window_size,
                })),
                window_min_size: Some(window_size),
                ..Default::default()
            },
            cx,
            |_, cx| cx.new(|ctx| MainWindow::new(device_name, receiver_image, ctx)),
        )
        .expect("Failed to open window");
    });

    Ok(())
}
