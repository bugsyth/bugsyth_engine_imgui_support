use bugsyth_engine::glium::backend::{Context, Facade};
use bugsyth_engine::glium::index::{self, PrimitiveType};
use bugsyth_engine::glium::program::ProgramChooserCreationError;
use bugsyth_engine::glium::texture::{
    ClientFormat, MipmapsOption, RawImage2d, TextureCreationError,
};
use bugsyth_engine::glium::uniforms::{
    MagnifySamplerFilter, MinifySamplerFilter, Sampler, SamplerBehavior, SamplerWrapFunction,
};
use bugsyth_engine::glium::{
    program, uniform, vertex, Blend, BlendingFunction, DrawError, DrawParameters, IndexBuffer,
    LinearBlendingFactor, Program, Rect, Surface, Texture2d, VertexBuffer,
};

use imgui::internal::RawWrapper;
use imgui::{BackendFlags, DrawCmd, DrawCmdParams, DrawData, TextureId, Textures};
use std::borrow::Cow;
use std::error::Error;
use std::fmt;
use std::rc::Rc;

#[derive(Clone, Debug)]
pub enum RendererError {
    Vertex(vertex::BufferCreationError),
    Index(index::BufferCreationError),
    Program(ProgramChooserCreationError),
    Texture(TextureCreationError),
    Draw(DrawError),
    BadTexture(TextureId),
}

impl Error for RendererError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        use self::RendererError::*;
        match *self {
            Vertex(ref e) => Some(e),
            Index(ref e) => Some(e),
            Program(ref e) => Some(e),
            Texture(ref e) => Some(e),
            Draw(ref e) => Some(e),
            BadTexture(_) => None,
        }
    }
}

impl fmt::Display for RendererError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::RendererError::*;
        match *self {
            Vertex(_) => write!(f, "Vertex buffer creation failed"),
            Index(_) => write!(f, "Index buffer creation failed"),
            Program(ref e) => write!(f, "Program creation failed: {}", e),
            Texture(_) => write!(f, "Texture creation failed"),
            Draw(ref e) => write!(f, "Drawing failed: {}", e),
            BadTexture(ref t) => write!(f, "Bad texture ID: {}", t.id()),
        }
    }
}

impl From<vertex::BufferCreationError> for RendererError {
    fn from(e: vertex::BufferCreationError) -> RendererError {
        RendererError::Vertex(e)
    }
}

impl From<index::BufferCreationError> for RendererError {
    fn from(e: index::BufferCreationError) -> RendererError {
        RendererError::Index(e)
    }
}

impl From<ProgramChooserCreationError> for RendererError {
    fn from(e: ProgramChooserCreationError) -> RendererError {
        RendererError::Program(e)
    }
}

impl From<TextureCreationError> for RendererError {
    fn from(e: TextureCreationError) -> RendererError {
        RendererError::Texture(e)
    }
}

impl From<DrawError> for RendererError {
    fn from(e: DrawError) -> RendererError {
        RendererError::Draw(e)
    }
}

pub struct Texture {
    pub texture: Rc<Texture2d>,
    pub sampler: SamplerBehavior,
}

pub struct Renderer {
    ctx: Rc<Context>,
    program: Program,
    font_texture: Texture,
    textures: Textures<Texture>,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct GliumDrawVert {
    pub pos: [f32; 2],
    pub uv: [f32; 2],
    pub col: [u8; 4],
}

// manual impl to avoid an allocation, and to reduce macro wonkiness.
impl bugsyth_engine::glium::vertex::Vertex for GliumDrawVert {
    #[inline]
    fn build_bindings() -> bugsyth_engine::glium::vertex::VertexFormat {
        use std::borrow::Cow::*;
        &[
            (
                Borrowed("pos"),
                0,
                -1,
                bugsyth_engine::glium::vertex::AttributeType::F32F32,
                false,
            ),
            (
                Borrowed("uv"),
                8,
                -1,
                bugsyth_engine::glium::vertex::AttributeType::F32F32,
                false,
            ),
            (
                Borrowed("col"),
                16,
                -1,
                bugsyth_engine::glium::vertex::AttributeType::U8U8U8U8,
                false,
            ),
        ]
    }
}

impl Renderer {
    /// Creates a new [`Renderer`].
    pub fn new<F: Facade>(ctx: &mut imgui::Context, facade: &F) -> Result<Renderer, RendererError> {
        let program = compile_default_program(facade)?;
        let font_texture = upload_font_texture(ctx.fonts(), facade.get_context())?;
        ctx.set_renderer_name(Some(format!(
            "imgui-glium-renderer {}",
            env!("CARGO_PKG_VERSION")
        )));
        ctx.io_mut()
            .backend_flags
            .insert(BackendFlags::RENDERER_HAS_VTX_OFFSET);
        Ok(Renderer {
            ctx: Rc::clone(facade.get_context()),
            program,
            font_texture,
            textures: Textures::new(),
        })
    }

    /// Creates a new [`Renderer`]
    #[deprecated(since = "0.13.0", note = "use `new` instead")]
    pub fn init<F: Facade>(
        ctx: &mut imgui::Context,
        facade: &F,
    ) -> Result<Renderer, RendererError> {
        Self::new(ctx, facade)
    }

    pub fn reload_font_texture(&mut self, ctx: &mut imgui::Context) -> Result<(), RendererError> {
        self.font_texture = upload_font_texture(ctx.fonts(), &self.ctx)?;
        Ok(())
    }
    pub fn textures(&mut self) -> &mut Textures<Texture> {
        &mut self.textures
    }
    fn lookup_texture(&self, texture_id: TextureId) -> Result<&Texture, RendererError> {
        if texture_id.id() == usize::MAX {
            Ok(&self.font_texture)
        } else if let Some(texture) = self.textures.get(texture_id) {
            Ok(texture)
        } else {
            Err(RendererError::BadTexture(texture_id))
        }
    }
    pub fn render<T: Surface>(
        &mut self,
        target: &mut T,
        draw_data: &DrawData,
    ) -> Result<(), RendererError> {
        let fb_width = draw_data.display_size[0] * draw_data.framebuffer_scale[0];
        let fb_height = draw_data.display_size[1] * draw_data.framebuffer_scale[1];
        if !(fb_width > 0.0 && fb_height > 0.0) {
            return Ok(());
        }
        let _ = self.ctx.insert_debug_marker("imgui-rs: starting rendering");
        let left = draw_data.display_pos[0];
        let right = draw_data.display_pos[0] + draw_data.display_size[0];
        let top = draw_data.display_pos[1];
        let bottom = draw_data.display_pos[1] + draw_data.display_size[1];
        let matrix = [
            [(2.0 / (right - left)), 0.0, 0.0, 0.0],
            [0.0, (2.0 / (top - bottom)), 0.0, 0.0],
            [0.0, 0.0, -1.0, 0.0],
            [
                (right + left) / (left - right),
                (top + bottom) / (bottom - top),
                0.0,
                1.0,
            ],
        ];
        let clip_off = draw_data.display_pos;
        let clip_scale = draw_data.framebuffer_scale;
        for draw_list in draw_data.draw_lists() {
            let vtx_buffer = VertexBuffer::immutable(&self.ctx, unsafe {
                draw_list.transmute_vtx_buffer::<GliumDrawVert>()
            })?;
            let idx_buffer = IndexBuffer::immutable(
                &self.ctx,
                PrimitiveType::TrianglesList,
                draw_list.idx_buffer(),
            )?;
            for cmd in draw_list.commands() {
                match cmd {
                    DrawCmd::Elements {
                        count,
                        cmd_params:
                            DrawCmdParams {
                                clip_rect,
                                texture_id,
                                vtx_offset,
                                idx_offset,
                                ..
                            },
                    } => {
                        let clip_rect = [
                            (clip_rect[0] - clip_off[0]) * clip_scale[0],
                            (clip_rect[1] - clip_off[1]) * clip_scale[1],
                            (clip_rect[2] - clip_off[0]) * clip_scale[0],
                            (clip_rect[3] - clip_off[1]) * clip_scale[1],
                        ];

                        if clip_rect[0] < fb_width
                            && clip_rect[1] < fb_height
                            && clip_rect[2] >= 0.0
                            && clip_rect[3] >= 0.0
                        {
                            let texture = self.lookup_texture(texture_id)?;

                            target.draw(
                                vtx_buffer
                                    .slice(vtx_offset..)
                                    .expect("Invalid vertex buffer range"),
                                idx_buffer
                                    .slice(idx_offset..(idx_offset + count))
                                    .expect("Invalid index buffer range"),
                                &self.program,
                                &uniform! {
                                    matrix: matrix,
                                    tex: Sampler(texture.texture.as_ref(), texture.sampler)
                                },
                                &DrawParameters {
                                    blend: Blend {
                                        alpha: BlendingFunction::Addition {
                                            source: LinearBlendingFactor::One,
                                            destination: LinearBlendingFactor::OneMinusSourceAlpha,
                                        },
                                        ..Blend::alpha_blending()
                                    },
                                    scissor: Some(Rect {
                                        left: f32::max(0.0, clip_rect[0]).floor() as u32,
                                        bottom: f32::max(0.0, fb_height - clip_rect[3]).floor()
                                            as u32,
                                        width: (clip_rect[2] - clip_rect[0]).abs().ceil() as u32,
                                        height: (clip_rect[3] - clip_rect[1]).abs().ceil() as u32,
                                    }),
                                    ..DrawParameters::default()
                                },
                            )?;
                        }
                    }
                    DrawCmd::ResetRenderState => (), // TODO
                    DrawCmd::RawCallback { callback, raw_cmd } => unsafe {
                        callback(draw_list.raw(), raw_cmd)
                    },
                }
            }
        }
        let _ = self.ctx.insert_debug_marker("imgui-rs: rendering finished");
        Ok(())
    }
}

fn upload_font_texture(
    fonts: &mut imgui::FontAtlas,
    ctx: &Rc<Context>,
) -> Result<Texture, RendererError> {
    let texture = fonts.build_rgba32_texture();
    let data = RawImage2d {
        data: Cow::Borrowed(texture.data),
        width: texture.width,
        height: texture.height,
        format: ClientFormat::U8U8U8U8,
    };
    let font_texture = Texture2d::with_mipmaps(ctx, data, MipmapsOption::NoMipmap)?;
    fonts.tex_id = TextureId::from(usize::MAX);
    Ok(Texture {
        texture: Rc::new(font_texture),
        sampler: SamplerBehavior {
            minify_filter: MinifySamplerFilter::Linear,
            magnify_filter: MagnifySamplerFilter::Linear,
            wrap_function: (
                SamplerWrapFunction::BorderClamp,
                SamplerWrapFunction::BorderClamp,
                SamplerWrapFunction::BorderClamp,
            ),
            ..Default::default()
        },
    })
}

fn compile_default_program<F: Facade>(facade: &F) -> Result<Program, ProgramChooserCreationError> {
    program!(
        facade,
        400 => {
            vertex: include_str!("shader/glsl_400.vert"),
            fragment: include_str!("shader/glsl_400.frag"),
            outputs_srgb: true,
        },
        150 => {
            vertex: include_str!("shader/glsl_150.vert"),
            fragment: include_str!("shader/glsl_150.frag"),
            outputs_srgb: true,
        },
        130 => {
            vertex: include_str!("shader/glsl_130.vert"),
            fragment: include_str!("shader/glsl_130.frag"),
            outputs_srgb: true,
        },
        110 => {
            vertex: include_str!("shader/glsl_110.vert"),
            fragment: include_str!("shader/glsl_110.frag"),
            outputs_srgb: true,
        },
        300 es => {
            vertex: include_str!("shader/glsles_300.vert"),
            fragment: include_str!("shader/glsles_300.frag"),
            outputs_srgb: true,
        },
        100 es => {
            vertex: include_str!("shader/glsles_100.vert"),
            fragment: include_str!("shader/glsles_100.frag"),
            outputs_srgb: true,
        },
    )
}
