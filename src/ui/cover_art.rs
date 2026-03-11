use image::load_from_memory;
use log::{info, warn};
use ratatui::{layout::Rect, Frame};
use ratatui_image::{
    picker::{Picker, ProtocolType},
    protocol::StatefulProtocol,
    Resize, StatefulImage,
};

struct CoverArtState {
    url: String,
    image: StatefulProtocol,
}

pub struct CoverArtRenderer {
    picker: Picker,
    state: Option<CoverArtState>,
}

impl CoverArtRenderer {
    pub fn new() -> Self {
        let picker = Picker::from_query_stdio().unwrap_or_else(|error| {
            warn!("cover art protocol detection failed, using halfblocks fallback: {error}");
            Picker::halfblocks()
        });

        info!(
            "cover art renderer using {:?} protocol",
            picker.protocol_type()
        );

        Self {
            picker,
            state: None,
        }
    }

    pub fn protocol_type(&self) -> ProtocolType {
        self.picker.protocol_type()
    }

    pub fn sync(&mut self, url: Option<&str>, bytes: Option<&[u8]>) {
        let Some(url) = url else {
            self.state = None;
            return;
        };

        let Some(bytes) = bytes else {
            if self.state.as_ref().map(|state| state.url.as_str()) != Some(url) {
                self.state = None;
            }
            return;
        };

        if self.state.as_ref().map(|state| state.url.as_str()) == Some(url) {
            return;
        }

        match load_from_memory(bytes) {
            Ok(image) => {
                self.state = Some(CoverArtState {
                    url: url.to_string(),
                    image: self.picker.new_resize_protocol(image),
                });
            }
            Err(error) => {
                warn!("cover art decode failed for {url}: {error}");
                self.state = None;
            }
        }
    }

    pub fn render(&mut self, frame: &mut Frame<'_>, area: Rect) -> bool {
        let Some(state) = self.state.as_mut() else {
            return false;
        };

        frame.render_stateful_widget(
            StatefulImage::default().resize(Resize::Fit(None)),
            area,
            &mut state.image,
        );
        true
    }
}
