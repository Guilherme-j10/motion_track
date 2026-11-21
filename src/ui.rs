use std::sync::Arc;

use gpui_kit::*;
use gpui_kit::{base::StyledExt, component::label::Label};

type Receiver = tokio::sync::watch::Receiver<Option<Arc<RenderImage>>>;

pub struct MainWindow {
    pub device: String,
    frame: Option<Arc<RenderImage>>,
}

impl MainWindow {
    pub fn new(device: String, mut receiver: Receiver, cx: &mut Context<Self>) -> Self {
        cx.spawn(async move |this, app| {
            while receiver.changed().await.is_ok() {
                let new_frame = receiver.borrow_and_update().clone();

                this.update(app, |view, cx| {
                    if let Some(old) = std::mem::replace(&mut view.frame, new_frame) {
                        cx.drop_image(old, None); // libera o frame antigo da GPU
                    }
                    cx.notify();
                })?;
            }

            anyhow::Ok(())
        })
        .detach();

        Self {
            device,
            frame: None,
        }
    }
}

impl Render for MainWindow {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .justify_start()
            .items_center()
            .bg(rgb(0x0a0a0a))
            .p(px(10.))
            .child(
                div()
                    .w_full()
                    .p(px(10.))
                    .bg(rgb(0x131313))
                    .flex()
                    .justify_start()
                    .items_center()
                    .child(
                        Label::new("Device selected:")
                            .text_color(rgb(0x8c8c8c))
                            .font_medium()
                            .mr(px(5.0)),
                    )
                    .child(Label::new(self.device.clone()).text_color(rgb(0xffffff))),
            )
            .child(match &self.frame {
                Some(frame) => img(frame.clone())
                    .mt(px(10.))
                    .object_fit(ObjectFit::Contain)
                    .into_any_element(),
                None => Label::new("Aguardando câmera...").into_any_element(),
            })
    }
}
