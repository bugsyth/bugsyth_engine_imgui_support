mod imgui_glium_renderer;
mod imgui_winit_support;

use std::time::Duration;

use glium::{
    glutin::surface::WindowSurface,
    winit::{event::WindowEvent, window::Window},
    Display, Surface,
};
use imgui::{FontConfig, FontGlyphRanges, FontSource, Ui};
use imgui_winit_support::HiDpiMode;

pub use {
    imgui::{Condition, Context},
    imgui_glium_renderer::Renderer,
    imgui_winit_support::WinitPlatform,
};

pub const FONT_SIZE: f32 = 13.0;

pub struct ImGui {
    pub context: Context,
    pub renderer: Renderer,
    pub platform: WinitPlatform,
}

impl ImGui {
    pub fn get_ui(&mut self, window: &Window) -> &mut Ui {
        self.platform
            .prepare_frame(self.context.io_mut(), window)
            .expect("Failed to prepare ImGui frame");
        self.context.frame()
    }
    pub fn render(&mut self, ui: &Ui, renderer: &mut impl Surface, window: &Window) {
        self.platform.prepare_render(ui, window);
        let draw_data = self.context.render();
        self.renderer
            .render(renderer, draw_data)
            .expect("Failed to render ImGui");
    }
    pub fn update_dt(&mut self, dt: f32) {
        self.context
            .io_mut()
            .update_delta_time(Duration::from_secs_f32(dt));
    }
    pub fn event(&mut self, window: &Window, event: &WindowEvent) {
        self.platform
            .handle_window_event(self.context.io_mut(), window, event);
    }
}

pub fn init<FInit>(window: &Window, display: &Display<WindowSurface>, mut startup: FInit) -> ImGui
where
    FInit: FnMut(&mut Context, &mut Renderer, &Display<WindowSurface>) + 'static,
{
    let mut imgui = create_context();
    let mut renderer = Renderer::new(&mut imgui, display).expect("Failed to initialize renderer");

    let mut platform = WinitPlatform::new(&mut imgui);
    platform.attach_window(imgui.io_mut(), window, HiDpiMode::Default);

    startup(&mut imgui, &mut renderer, &display);
    ImGui {
        context: imgui,
        renderer,
        platform,
    }
}

/// Creates the imgui context
fn create_context() -> imgui::Context {
    let mut imgui = Context::create();
    // Fixed font size. Note imgui_winit_support uses "logical
    // pixels", which are physical pixels scaled by the devices
    // scaling factor. Meaning, 13.0 pixels should look the same size
    // on two different screens, and thus we do not need to scale this
    // value (as the scaling is handled by winit)
    imgui.fonts().add_font(&[
        FontSource::TtfData {
            data: include_bytes!("../resources/Roboto-Regular.ttf"),
            size_pixels: FONT_SIZE,
            config: Some(FontConfig {
                // As imgui-glium-renderer isn't gamma-correct with
                // it's font rendering, we apply an arbitrary
                // multiplier to make the font a bit "heavier". With
                // default imgui-glow-renderer this is unnecessary.
                rasterizer_multiply: 1.5,
                // Oversampling font helps improve text rendering at
                // expense of larger font atlas texture.
                oversample_h: 4,
                oversample_v: 4,
                ..FontConfig::default()
            }),
        },
        FontSource::TtfData {
            data: include_bytes!("../resources/mplus-1p-regular.ttf"),
            size_pixels: FONT_SIZE,
            config: Some(FontConfig {
                // Oversampling font helps improve text rendering at
                // expense of larger font atlas texture.
                oversample_h: 4,
                oversample_v: 4,
                // Range of glyphs to rasterize
                glyph_ranges: FontGlyphRanges::japanese(),
                ..FontConfig::default()
            }),
        },
    ]);
    imgui.set_ini_filename(None);

    imgui
}
