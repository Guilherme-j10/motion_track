use std::sync::Arc;

use anyhow::anyhow;
use image::Frame;
use nokhwa::{
    Camera,
    pixel_format::{RgbAFormat, RgbFormat},
    utils::{ApiBackend, RequestedFormat},
};
use tokio_util::sync::CancellationToken;
use tracing::{Level, info};
use tracing_subscriber::FmtSubscriber;
use rayon::prelude::*;

use gpui_kit::*;

use crate::ui::MainWindow;

mod ui;

fn main() -> anyhow::Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let application_token = CancellationToken::new();
    let supported_devices = nokhwa::query(ApiBackend::Auto)
        .map_err(|e| anyhow!("Failed to list supported devices: {e}"))?;

    let device_index = supported_devices[0].index().to_owned();
    let device_name = supported_devices[0].human_name();
    let capture_token = application_token.clone();

    info!("Trying device: {}", device_name);

    let (sender_image, receiver_image) = tokio::sync::watch::channel::<Option<Arc<RenderImage>>>(None);

    std::thread::spawn(move || {
        let requested = RequestedFormat::new::<RgbFormat>(
            nokhwa::utils::RequestedFormatType::AbsoluteHighestFrameRate,
        );
        let mut device = Camera::new(device_index, requested).unwrap();

        loop {
            if capture_token.is_cancelled() {
                break;
            }

            let frame = device
                .frame()
                .map_err(|e| anyhow!("Failed to get a frame from the camera: {e}"))
                .unwrap();
            let mut image = frame.decode_image::<RgbAFormat>().unwrap();
            image::imageops::flip_horizontal_in_place(&mut image);

            for chunk in image.chunks_exact_mut(4) {
                // keep a copy in rgb to mediapipe
                chunk.swap(0, 2); // convert from rgba to bgra
            }

            let to_frame = Frame::new(image);
            let render = RenderImage::new(vec![to_frame]);

            sender_image
                .send(Some(Arc::new(render)))
                .expect("Failed to send image to UI");
        }
    });

    application().with_assets(assets::Assets).run(|cx| {
        init(cx);

        let mut position = Point {
            x: px(10.0),
            y: px(10.0),
        };
        let window_size = Size {
            width: px(495.0),
            height: px(335.0),
        };

        if let Some(display) = cx.primary_display() {
            let display_bounds = display.bounds().size;

            position.x = px(display_bounds.width.as_f32() / 2. - (window_size.width.as_f32() / 2.));
            position.y =
                px(display_bounds.height.as_f32() / 2. - (window_size.height.as_f32() / 2.));
        }

        info!("{:?}", position);

        open_window(
            WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some("Motion track control app".into()),
                    appears_transparent: false,
                    ..Default::default()
                }),
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: position,
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
