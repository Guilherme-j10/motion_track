use std::{sync::Arc};

use anyhow::anyhow;
use nokhwa::{
    Camera,
    pixel_format::RgbFormat,
    utils::{RequestedFormat, RequestedFormatType::AbsoluteHighestFrameRate},
};
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;
use tracing::{Level, error};
use tracing_subscriber::FmtSubscriber;

use gpui_kit::*;

use crate::{capture::Capture, ui::MainWindow};

mod capture;
mod ui;

fn main() -> anyhow::Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let application_token = CancellationToken::new();

    let capture_instance = Capture::new()?;
    let (device_name, device_index) = capture_instance.set_camera(0)?;

    let (sender_image, receiver_image) = watch::channel::<Option<Arc<RenderImage>>>(None);
    let (last_frame_sender, mut last_frame_receiver) = watch::channel(None);

    let capture_token = application_token.clone();

    std::thread::spawn(move || {
        let req = RequestedFormat::new::<RgbFormat>(AbsoluteHighestFrameRate);
        let mut camera = Camera::new(device_index.to_owned(), req)
            .map_err(|e| anyhow!("Failed to build cam instance: {e}"))
            .unwrap();

        loop {
            if capture_token.is_cancelled() {
                break;
            }

            match capture_instance.process_frame(&mut camera, &last_frame_sender) {
                Ok(frame) => {
                    sender_image
                        .send(Some(Arc::new(frame)))
                        .expect("Failed to send image to UI");
                }
                Err(e) => match e {
                    _ => {
                        capture_token.cancel();
                        error!("{e}");
                        break;
                    }
                },
            }
        }
    });

    let infer_token = application_token.clone();

    std::thread::spawn(move || {
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
                    println!("Frame: {}", current_frame.unwrap().len());
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
        }).detach();

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
