//! Ganesh over the host-created WebGL2 context. Context operations stay inside this adapter.
use crate::{Error, Scene, dimensions};
use skia_safe::{
    self as sk, Color, ColorType, Surface,
    gpu::{
        self, DirectContext, SurfaceOrigin, backend_render_targets, direct_contexts,
        gl::FramebufferInfo,
    },
};
pub struct WebGl {
    // Surface is dropped before its owning context.
    surface: Surface,
    context: DirectContext,
}
impl WebGl {
    /// The host has made a WebGL2 context current, with a stencil buffer and RGBA8 default target.
    pub fn new(w: u32, h: u32) -> Result<Self, Error> {
        dimensions(w, h)?;
        let interface =
            gpu::gl::Interface::new_native().ok_or(Error::Unsupported("WebGL2 context"))?;
        let mut context = direct_contexts::make_gl(interface, None)
            .ok_or(Error::Unsupported("Ganesh context"))?;
        let surface = surface(&mut context, w, h)?;
        Ok(Self { surface, context })
    }
    pub fn resize(&mut self, w: u32, h: u32) -> Result<(), Error> {
        let next = surface(&mut self.context, w, h)?;
        self.surface = next;
        Ok(())
    }
    pub fn draw(&mut self, scene: Option<&Scene>) {
        self.surface.canvas().clear(Color::TRANSPARENT);
        if let Some(scene) = scene {
            scene.draw(self.surface.canvas());
        }
        self.context.flush_and_submit();
    }
    pub fn abandon(&mut self) {
        self.context.abandon();
    }
    pub fn pixels(&mut self) -> Result<Vec<u8>, Error> {
        let w = self.surface.width();
        let h = self.surface.height();
        let info = sk::ImageInfo::new((w, h), ColorType::RGBA8888, sk::AlphaType::Unpremul, None);
        let mut pixels = vec![0; w as usize * h as usize * 4];
        if !self
            .surface
            .read_pixels(&info, &mut pixels, w as usize * 4, (0, 0))
        {
            return Err(Error::Encode);
        }
        Ok(pixels)
    }
}
fn surface(c: &mut DirectContext, w: u32, h: u32) -> Result<Surface, Error> {
    dimensions(w, h)?;
    let info = FramebufferInfo {
        fboid: 0,
        format: 0x8058,
        ..Default::default()
    };
    let target = backend_render_targets::make_gl((w as i32, h as i32), 0, 8, info);
    gpu::surfaces::wrap_backend_render_target(
        c,
        &target,
        SurfaceOrigin::BottomLeft,
        ColorType::RGBA8888,
        None,
        None,
    )
    .ok_or(Error::InvalidSize)
}
