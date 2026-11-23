use std::sync::Arc;

use anyhow::bail;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

use gpui_kit::*;

use crate::{
    hardware::camera::Capture, inference::prepare_frame, ui::MainWindow, utils::LastFrameType,
};

mod hardware;
mod inference;
mod ui;
mod utils;

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
        let (mut camera, cam_resolution) = match Capture::camera_handle(device_index) {
            Ok(cam) => cam,
            Err(e) => {
                capture_token.cancel();
                bail!(e)
            }
        };

        loop {
            if capture_token.is_cancelled() {
                break Ok(());
            }

            match capture_instance.process_frame(&mut camera, &last_frame_sender, cam_resolution) {
                Ok(frame) => {
                    if let Err(e) = sender_image.send(Some(Arc::new(frame))) {
                        capture_token.cancel();
                        bail!("Error in send last frame to inference layer: {e}");
                    }
                }
                Err(e) => {
                    capture_token.cancel();
                    bail!("{e}");
                }
            }
        }
    });

    let infer_token = application_token.clone();

    std::thread::spawn(move || {
        loop {
            if infer_token.is_cancelled() {
                break Ok(());
            }

            match last_frame_receiver.has_changed() {
                Ok(status) => {
                    if !status {
                        continue;
                    }

                    let last_frame = last_frame_receiver.borrow_and_update().clone();
                    let _ = prepare_frame(last_frame);
                }
                Err(e) => {
                    infer_token.cancel();
                    bail!("Receiver last frame channel error: {e}");
                }
            }
        }
    });

    let ui_token = application_token.clone();

    application().with_assets(assets::Assets).run(|cx| {
        init(cx);

        cx.spawn(async move |app| {
            ui_token.cancelled().await;
            app.update(|app_ctx| app_ctx.quit());
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
