use std::{
    collections::HashMap,
    ffi::CString,
    num::NonZeroU32,
};

use anyhow::{anyhow, bail, Context, Result};
use bytemuck::{Pod, Zeroable};
use fontdue::{
    layout::{CoordinateSystem, GlyphRasterConfig, Layout, LayoutSettings, TextStyle},
    Font,
};
use glow::HasContext as _;
use glutin::{
    config::{Config, ConfigTemplateBuilder},
    context::{
        ContextApi, ContextAttributesBuilder, NotCurrentContext, PossiblyCurrentContext, Version,
    },
    display::GetGlDisplay as _,
    prelude::*,
    surface::{Surface, SurfaceAttributesBuilder, SwapInterval, WindowSurface},
};
use glutin_winit::DisplayBuilder;
use raw_window_handle::HasWindowHandle as _;
use winit::{
    dpi::PhysicalSize,
    event_loop::EventLoop,
    window::{Window, WindowAttributes, WindowLevel},
};

use crate::ui::{Color, CornerRadii, DrawCommand, Point, Rect, UiScene, BASE_WINDOW_HEIGHT, BASE_WINDOW_WIDTH};

const FONT_BYTES: &[u8] = include_bytes!("assets\\Ubuntu-Light.ttf");
const CORNER_SEGMENTS: usize = 8;

pub struct GlWindow {
    pub window: Window,
    surface: Surface<WindowSurface>,
    context: PossiblyCurrentContext,
}

impl GlWindow {
    pub fn resize(&self, size: PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            return;
        }
        self.surface.resize(
            &self.context,
            NonZeroU32::new(size.width).unwrap(),
            NonZeroU32::new(size.height).unwrap(),
        );
    }

    pub fn swap_buffers(&self) -> Result<()> {
        self.surface
            .swap_buffers(&self.context)
            .context("swapping OpenGL buffers")
    }
}

pub fn create_gl_window<T>(event_loop: &EventLoop<T>) -> Result<(GlWindow, Renderer)> {
    let window_attributes = WindowAttributes::default()
        .with_title("Keyboim")
        .with_transparent(true)
        .with_decorations(false)
        .with_resizable(false)
        .with_visible(false)
        .with_window_level(WindowLevel::AlwaysOnTop)
        .with_inner_size(winit::dpi::LogicalSize::new(
            BASE_WINDOW_WIDTH as f64,
            BASE_WINDOW_HEIGHT as f64,
        ));

    let template = ConfigTemplateBuilder::new()
        .with_alpha_size(8)
        .with_transparency(true);
    let display_builder = DisplayBuilder::new().with_window_attributes(Some(window_attributes));

    let (window, config) = display_builder
        .build(event_loop, template, pick_gl_config)
        .map_err(|error| anyhow!("creating window and GL config: {error}"))?;
    let window = window.context("glutin did not return a window")?;

    let raw_window_handle = window.window_handle().context("getting window handle")?;
    let raw_window_handle = raw_window_handle.as_raw();
    let gl_display = config.display();

    let context_attributes = ContextAttributesBuilder::new()
        .with_context_api(ContextApi::OpenGl(Some(Version::new(3, 3))))
        .build(Some(raw_window_handle));
    let fallback_context_attributes = ContextAttributesBuilder::new()
        .with_context_api(ContextApi::Gles(None))
        .build(Some(raw_window_handle));

    let not_current = unsafe {
        gl_display
            .create_context(&config, &context_attributes)
            .or_else(|_| gl_display.create_context(&config, &fallback_context_attributes))
            .context("creating GL context")?
    };

    let size = window.inner_size();
    let surface_attributes = SurfaceAttributesBuilder::<WindowSurface>::new().build(
        raw_window_handle,
        NonZeroU32::new(size.width.max(1)).unwrap(),
        NonZeroU32::new(size.height.max(1)).unwrap(),
    );
    let surface = unsafe {
        gl_display
            .create_window_surface(&config, &surface_attributes)
            .context("creating GL surface")?
    };
    let context = make_current(not_current, &surface)?;
    surface
        .set_swap_interval(&context, SwapInterval::Wait(NonZeroU32::new(1).unwrap()))
        .ok();

    let gl = unsafe {
        glow::Context::from_loader_function(|symbol| {
            let symbol = CString::new(symbol).expect("valid GL symbol");
            gl_display.get_proc_address(&symbol) as *const _
        })
    };

    let renderer = unsafe { Renderer::new(gl) }?;
    Ok((
        GlWindow {
            window,
            surface,
            context,
        },
        renderer,
    ))
}

fn pick_gl_config(configs: Box<dyn Iterator<Item = Config> + '_>) -> Config {
    configs
        .max_by_key(|config| {
            let transparency = i32::from(config.supports_transparency().unwrap_or(false));
            transparency * 1_000 + config.num_samples() as i32
        })
        .expect("at least one GL config")
}

fn make_current(
    context: NotCurrentContext,
    surface: &Surface<WindowSurface>,
) -> Result<PossiblyCurrentContext> {
    context.make_current(surface).context("making GL context current")
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SolidVertex {
    position: [f32; 2],
    color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct TextVertex {
    position: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
}

pub struct PreparedScene {
    pub size: (u32, u32),
    pub clear_color: Color,
    solid_vertices: Vec<SolidVertex>,
    solid_indices: Vec<u32>,
    text_batches: Vec<TextBatch>,
}

struct TextBatch {
    vertices: Vec<TextVertex>,
    indices: Vec<u32>,
}

pub struct Renderer {
    gl: glow::Context,
    solid_program: glow::NativeProgram,
    text_program: glow::NativeProgram,
    solid_vao: glow::NativeVertexArray,
    solid_vbo: glow::NativeBuffer,
    solid_ebo: glow::NativeBuffer,
    text_vao: glow::NativeVertexArray,
    text_vbo: glow::NativeBuffer,
    text_ebo: glow::NativeBuffer,
    atlas_texture: glow::NativeTexture,
    atlas: GlyphAtlas,
}

impl Renderer {
    unsafe fn new(gl: glow::Context) -> Result<Self> {
        let solid_program = create_program(
            &gl,
            SOLID_VERTEX_SHADER,
            SOLID_FRAGMENT_SHADER,
            "solid",
        )?;
        let text_program = create_program(&gl, TEXT_VERTEX_SHADER, TEXT_FRAGMENT_SHADER, "text")?;

        let solid_vao = gl_result(gl.create_vertex_array(), "creating solid VAO")?;
        let solid_vbo = gl_result(gl.create_buffer(), "creating solid VBO")?;
        let solid_ebo = gl_result(gl.create_buffer(), "creating solid EBO")?;
        gl.bind_vertex_array(Some(solid_vao));
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(solid_vbo));
        gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(solid_ebo));
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_f32(
            0,
            2,
            glow::FLOAT,
            false,
            std::mem::size_of::<SolidVertex>() as i32,
            0,
        );
        gl.enable_vertex_attrib_array(1);
        gl.vertex_attrib_pointer_f32(
            1,
            4,
            glow::FLOAT,
            false,
            std::mem::size_of::<SolidVertex>() as i32,
            8,
        );

        let text_vao = gl_result(gl.create_vertex_array(), "creating text VAO")?;
        let text_vbo = gl_result(gl.create_buffer(), "creating text VBO")?;
        let text_ebo = gl_result(gl.create_buffer(), "creating text EBO")?;
        gl.bind_vertex_array(Some(text_vao));
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(text_vbo));
        gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(text_ebo));
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_f32(
            0,
            2,
            glow::FLOAT,
            false,
            std::mem::size_of::<TextVertex>() as i32,
            0,
        );
        gl.enable_vertex_attrib_array(1);
        gl.vertex_attrib_pointer_f32(
            1,
            2,
            glow::FLOAT,
            false,
            std::mem::size_of::<TextVertex>() as i32,
            8,
        );
        gl.enable_vertex_attrib_array(2);
        gl.vertex_attrib_pointer_f32(
            2,
            4,
            glow::FLOAT,
            false,
            std::mem::size_of::<TextVertex>() as i32,
            16,
        );

        let atlas_texture = gl_result(gl.create_texture(), "creating atlas texture")?;
        gl.bind_texture(glow::TEXTURE_2D, Some(atlas_texture));
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::LINEAR as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::LINEAR as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE as i32);
        gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::R8 as i32,
            2048,
            2048,
            0,
            glow::RED,
            glow::UNSIGNED_BYTE,
            glow::PixelUnpackData::Slice(None),
        );

        gl.enable(glow::BLEND);
        gl.blend_func_separate(
            glow::SRC_ALPHA,
            glow::ONE_MINUS_SRC_ALPHA,
            glow::ONE,
            glow::ONE_MINUS_SRC_ALPHA,
        );

        Ok(Self {
            gl,
            solid_program,
            text_program,
            solid_vao,
            solid_vbo,
            solid_ebo,
            text_vao,
            text_vbo,
            text_ebo,
            atlas_texture,
            atlas: GlyphAtlas::new()?,
        })
    }

    pub fn prepare_scene(&mut self, scene: &UiScene) -> Result<PreparedScene> {
        let mut solid_vertices = Vec::new();
        let mut solid_indices = Vec::new();
        let mut text_batches = Vec::new();

        for command in &scene.commands {
            match command {
                DrawCommand::RoundedRectFill { rect, radii, color } => {
                    push_rounded_rect_fill(&mut solid_vertices, &mut solid_indices, *rect, *radii, *color);
                }
                DrawCommand::RoundedRectStroke {
                    rect,
                    radii,
                    color,
                    width,
                } => {
                    push_rounded_rect_stroke(
                        &mut solid_vertices,
                        &mut solid_indices,
                        *rect,
                        *radii,
                        *color,
                        *width,
                    );
                }
                DrawCommand::Polygon { points, color } => {
                    push_polygon(&mut solid_vertices, &mut solid_indices, points, *color);
                }
                DrawCommand::Polyline {
                    points,
                    color,
                    width,
                    closed,
                } => {
                    push_polyline(
                        &mut solid_vertices,
                        &mut solid_indices,
                        points,
                        *color,
                        *width,
                        *closed,
                    );
                }
                DrawCommand::Text {
                    position,
                    text,
                    size,
                    color,
                } => {
                    let mut text_vertices = Vec::new();
                    let mut text_indices = Vec::new();
                    self.push_text(
                        &mut text_vertices,
                        &mut text_indices,
                        scene.physical_size,
                        *position,
                        text,
                        *size,
                        *color,
                    )?;
                    if !text_indices.is_empty() {
                        text_batches.push(TextBatch {
                            vertices: text_vertices,
                            indices: text_indices,
                        });
                    }
                }
            }
        }

        Ok(PreparedScene {
            size: scene.physical_size,
            clear_color: scene.clear_color,
            solid_vertices,
            solid_indices,
            text_batches,
        })
    }

    pub fn render(&self, prepared: &PreparedScene) -> Result<()> {
        unsafe {
            self.gl.viewport(0, 0, prepared.size.0 as i32, prepared.size.1 as i32);
            self.gl.clear_color(
                prepared.clear_color.r as f32 / 255.0,
                prepared.clear_color.g as f32 / 255.0,
                prepared.clear_color.b as f32 / 255.0,
                prepared.clear_color.a as f32 / 255.0,
            );
            self.gl.clear(glow::COLOR_BUFFER_BIT);

            if !prepared.solid_indices.is_empty() {
                self.gl.use_program(Some(self.solid_program));
                upload_buffer(
                    &self.gl,
                    glow::ARRAY_BUFFER,
                    self.solid_vbo,
                    bytemuck::cast_slice(&prepared.solid_vertices),
                );
                upload_buffer(
                    &self.gl,
                    glow::ELEMENT_ARRAY_BUFFER,
                    self.solid_ebo,
                    bytemuck::cast_slice(&prepared.solid_indices),
                );
                self.gl.bind_vertex_array(Some(self.solid_vao));
                if let Some(location) = self.gl.get_uniform_location(self.solid_program, "u_screen_size")
                {
                    self.gl
                        .uniform_2_f32(Some(&location), prepared.size.0 as f32, prepared.size.1 as f32);
                }
                self.gl.draw_elements(
                    glow::TRIANGLES,
                    prepared.solid_indices.len() as i32,
                    glow::UNSIGNED_INT,
                    0,
                );
            }

            if !prepared.text_batches.is_empty() {
                self.gl.use_program(Some(self.text_program));
                self.gl.active_texture(glow::TEXTURE0);
                self.gl.bind_texture(glow::TEXTURE_2D, Some(self.atlas_texture));
                self.gl.bind_vertex_array(Some(self.text_vao));
                if let Some(location) = self.gl.get_uniform_location(self.text_program, "u_screen_size")
                {
                    self.gl
                        .uniform_2_f32(Some(&location), prepared.size.0 as f32, prepared.size.1 as f32);
                }
                if let Some(location) = self.gl.get_uniform_location(self.text_program, "u_texture") {
                    self.gl.uniform_1_i32(Some(&location), 0);
                }
                for batch in &prepared.text_batches {
                    upload_buffer(
                        &self.gl,
                        glow::ARRAY_BUFFER,
                        self.text_vbo,
                        bytemuck::cast_slice(&batch.vertices),
                    );
                    upload_buffer(
                        &self.gl,
                        glow::ELEMENT_ARRAY_BUFFER,
                        self.text_ebo,
                        bytemuck::cast_slice(&batch.indices),
                    );
                    self.gl.draw_elements(
                        glow::TRIANGLES,
                        batch.indices.len() as i32,
                        glow::UNSIGNED_INT,
                        0,
                    );
                }
            }

            self.gl.bind_vertex_array(None);
            self.gl.use_program(None);
        }
        Ok(())
    }

    pub fn read_pixels(&self, size: (u32, u32)) -> Vec<u8> {
        let mut bytes = vec![0u8; (size.0 * size.1 * 4) as usize];
        unsafe {
            self.gl.read_pixels(
                0,
                0,
                size.0 as i32,
                size.1 as i32,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelPackData::Slice(Some(&mut bytes)),
            );
        }
        flip_rgba_rows(&mut bytes, size.0 as usize, size.1 as usize);
        bytes
    }

    fn push_text(
        &mut self,
        vertices: &mut Vec<TextVertex>,
        indices: &mut Vec<u32>,
        screen_size: (u32, u32),
        position: Point,
        text: &str,
        size: f32,
        color: Color,
    ) -> Result<()> {
        let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
        layout.reset(&LayoutSettings {
            x: position.x,
            y: position.y,
            ..LayoutSettings::default()
        });
        layout.append(&[&self.atlas.font], &TextStyle::new(text, size, 0));

        for glyph in layout.glyphs() {
            let cached = self
                .atlas
                .ensure_glyph(&self.gl, self.atlas_texture, glyph.key)?;
            if cached.width == 0 || cached.height == 0 {
                continue;
            }

            let x = glyph.x;
            let y = glyph.y;
            let w = cached.width as f32;
            let h = cached.height as f32;

            if x > screen_size.0 as f32 || y > screen_size.1 as f32 {
                continue;
            }

            let base = vertices.len() as u32;
            let tint = color_to_f32(color);
            vertices.extend_from_slice(&[
                TextVertex {
                    position: [x, y],
                    uv: [cached.uv_min[0], cached.uv_min[1]],
                    color: tint,
                },
                TextVertex {
                    position: [x + w, y],
                    uv: [cached.uv_max[0], cached.uv_min[1]],
                    color: tint,
                },
                TextVertex {
                    position: [x + w, y + h],
                    uv: [cached.uv_max[0], cached.uv_max[1]],
                    color: tint,
                },
                TextVertex {
                    position: [x, y + h],
                    uv: [cached.uv_min[0], cached.uv_max[1]],
                    color: tint,
                },
            ]);
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        Ok(())
    }
}

unsafe fn upload_buffer(
    gl: &glow::Context,
    target: u32,
    buffer: glow::NativeBuffer,
    bytes: &[u8],
) {
    gl.bind_buffer(target, Some(buffer));
    gl.buffer_data_u8_slice(target, bytes, glow::DYNAMIC_DRAW);
}

fn gl_result<T>(result: std::result::Result<T, String>, context: &str) -> Result<T> {
    result.map_err(|error| anyhow!("{context}: {error}"))
}

fn create_program(
    gl: &glow::Context,
    vertex_shader_source: &str,
    fragment_shader_source: &str,
    label: &str,
) -> Result<glow::NativeProgram> {
    unsafe {
        let program = gl_result(gl.create_program(), &format!("creating {label} program"))?;
        let shaders = [
            (glow::VERTEX_SHADER, vertex_shader_source, "vertex"),
            (glow::FRAGMENT_SHADER, fragment_shader_source, "fragment"),
        ];

        let mut compiled = Vec::new();
        for (shader_type, source, stage) in shaders {
            let shader = gl_result(
                gl.create_shader(shader_type),
                &format!("creating {label} {stage} shader"),
            )?;
            gl.shader_source(shader, source);
            gl.compile_shader(shader);
            if !gl.get_shader_compile_status(shader) {
                let log = gl.get_shader_info_log(shader);
                gl.delete_shader(shader);
                bail!("{label} {stage} shader compile failed: {log}");
            }
            gl.attach_shader(program, shader);
            compiled.push(shader);
        }

        gl.link_program(program);
        if !gl.get_program_link_status(program) {
            let log = gl.get_program_info_log(program);
            for shader in compiled {
                gl.detach_shader(program, shader);
                gl.delete_shader(shader);
            }
            gl.delete_program(program);
            bail!("{label} program link failed: {log}");
        }

        for shader in compiled {
            gl.detach_shader(program, shader);
            gl.delete_shader(shader);
        }

        Ok(program)
    }
}

fn flip_rgba_rows(bytes: &mut [u8], width: usize, height: usize) {
    let stride = width * 4;
    for y in 0..height / 2 {
        let top = y * stride;
        let bottom = (height - 1 - y) * stride;
        for offset in 0..stride {
            bytes.swap(top + offset, bottom + offset);
        }
    }
}

struct GlyphAtlas {
    font: Font,
    cache: HashMap<GlyphCacheKey, CachedGlyph>,
    cursor_x: u32,
    cursor_y: u32,
    row_height: u32,
    size: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct GlyphCacheKey {
    glyph_index: u16,
    px_bits: u32,
}

#[derive(Clone, Copy)]
struct CachedGlyph {
    uv_min: [f32; 2],
    uv_max: [f32; 2],
    width: u32,
    height: u32,
}

impl GlyphAtlas {
    fn new() -> Result<Self> {
        let font = Font::from_bytes(FONT_BYTES, fontdue::FontSettings::default())
            .map_err(|error| anyhow!("loading embedded Ubuntu-Light font: {error}"))?;
        Ok(Self {
            font,
            cache: HashMap::new(),
            cursor_x: 1,
            cursor_y: 1,
            row_height: 0,
            size: 2048,
        })
    }

    fn ensure_glyph(
        &mut self,
        gl: &glow::Context,
        texture: glow::NativeTexture,
        config: GlyphRasterConfig,
    ) -> Result<CachedGlyph> {
        let key = GlyphCacheKey {
            glyph_index: config.glyph_index,
            px_bits: config.px.to_bits(),
        };

        if let Some(cached) = self.cache.get(&key) {
            return Ok(*cached);
        }

        let (metrics, bitmap) = self.font.rasterize_indexed(config.glyph_index, config.px);
        if metrics.width == 0 || metrics.height == 0 {
            let glyph = CachedGlyph {
                uv_min: [0.0, 0.0],
                uv_max: [0.0, 0.0],
                width: 0,
                height: 0,
            };
            self.cache.insert(key, glyph);
            return Ok(glyph);
        }

        let padding = 1u32;
        let glyph_width = metrics.width as u32;
        let glyph_height = metrics.height as u32;
        if self.cursor_x + glyph_width + padding >= self.size {
            self.cursor_x = 1;
            self.cursor_y += self.row_height + padding;
            self.row_height = 0;
        }
        if self.cursor_y + glyph_height + padding >= self.size {
            bail!("font atlas exhausted");
        }

        let x = self.cursor_x;
        let y = self.cursor_y;
        self.cursor_x += glyph_width + padding;
        self.row_height = self.row_height.max(glyph_height);

        unsafe {
            gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            gl.tex_sub_image_2d(
                glow::TEXTURE_2D,
                0,
                x as i32,
                y as i32,
                glyph_width as i32,
                glyph_height as i32,
                glow::RED,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(Some(&bitmap)),
            );
        }

        let cached = CachedGlyph {
            uv_min: [x as f32 / self.size as f32, y as f32 / self.size as f32],
            uv_max: [
                (x + glyph_width) as f32 / self.size as f32,
                (y + glyph_height) as f32 / self.size as f32,
            ],
            width: glyph_width,
            height: glyph_height,
        };
        self.cache.insert(key, cached);
        Ok(cached)
    }
}

fn push_rounded_rect_fill(
    vertices: &mut Vec<SolidVertex>,
    indices: &mut Vec<u32>,
    rect: Rect,
    _radii: CornerRadii,
    color: Color,
) {
    let points = vec![
        rect.min,
        Point {
            x: rect.max.x,
            y: rect.min.y,
        },
        rect.max,
        Point {
            x: rect.min.x,
            y: rect.max.y,
        },
    ];
    push_polygon(vertices, indices, &points, color);
}

fn push_rounded_rect_stroke(
    vertices: &mut Vec<SolidVertex>,
    indices: &mut Vec<u32>,
    rect: Rect,
    radii: CornerRadii,
    color: Color,
    width: f32,
) {
    let half = width * 0.5;
    let outer = rounded_rect_contour_points(rect, radii, half);
    let inner = rounded_rect_contour_points(rect, radii, -half);
    if outer.len() != inner.len() || outer.len() < 2 {
        return;
    }

    let tint = color_to_f32(color);
    let base = vertices.len() as u32;
    for index in 0..outer.len() {
        vertices.push(SolidVertex {
            position: [outer[index].x, outer[index].y],
            color: tint,
        });
        vertices.push(SolidVertex {
            position: [inner[index].x, inner[index].y],
            color: tint,
        });
    }

    for index in 0..outer.len() {
        let next = (index + 1) % outer.len();
        let current_outer = base + (index as u32 * 2);
        let current_inner = current_outer + 1;
        let next_outer = base + (next as u32 * 2);
        let next_inner = next_outer + 1;
        indices.extend_from_slice(&[
            current_outer,
            current_inner,
            next_inner,
            current_outer,
            next_inner,
            next_outer,
        ]);
    }
}

fn rounded_rect_contour_points(rect: Rect, radii: CornerRadii, offset: f32) -> Vec<Point> {
    let inset_min_x = (rect.min.x + offset).min(rect.max.x);
    let inset_min_y = (rect.min.y + offset).min(rect.max.y);
    let inset_max_x = (rect.max.x - offset).max(inset_min_x);
    let inset_max_y = (rect.max.y - offset).max(inset_min_y);
    let contour_rect = Rect {
        min: Point {
            x: inset_min_x,
            y: inset_min_y,
        },
        max: Point {
            x: inset_max_x,
            y: inset_max_y,
        },
    };

    let contour_radii = CornerRadii {
        top_left: (radii.top_left + offset).max(0.0),
        top_right: (radii.top_right + offset).max(0.0),
        bottom_right: (radii.bottom_right + offset).max(0.0),
        bottom_left: (radii.bottom_left + offset).max(0.0),
    };

    let mut points = Vec::with_capacity((CORNER_SEGMENTS + 1) * 4);
    append_contour_arc(
        &mut points,
        Point {
            x: contour_rect.max.x - contour_radii.top_right,
            y: contour_rect.min.y + contour_radii.top_right,
        },
        contour_radii.top_right,
        -90.0,
        0.0,
    );
    append_contour_arc(
        &mut points,
        Point {
            x: contour_rect.max.x - contour_radii.bottom_right,
            y: contour_rect.max.y - contour_radii.bottom_right,
        },
        contour_radii.bottom_right,
        0.0,
        90.0,
    );
    append_contour_arc(
        &mut points,
        Point {
            x: contour_rect.min.x + contour_radii.bottom_left,
            y: contour_rect.max.y - contour_radii.bottom_left,
        },
        contour_radii.bottom_left,
        90.0,
        180.0,
    );
    append_contour_arc(
        &mut points,
        Point {
            x: contour_rect.min.x + contour_radii.top_left,
            y: contour_rect.min.y + contour_radii.top_left,
        },
        contour_radii.top_left,
        180.0,
        270.0,
    );
    points
}

fn append_contour_arc(
    points: &mut Vec<Point>,
    center: Point,
    radius: f32,
    start_deg: f32,
    end_deg: f32,
) {
    for step in 0..=CORNER_SEGMENTS {
        let t = step as f32 / CORNER_SEGMENTS as f32;
        let angle = (start_deg + (end_deg - start_deg) * t).to_radians();
        points.push(Point {
            x: center.x + angle.cos() * radius,
            y: center.y + angle.sin() * radius,
        });
    }
}

fn push_polygon(
    vertices: &mut Vec<SolidVertex>,
    indices: &mut Vec<u32>,
    points: &[Point],
    color: Color,
) {
    if points.len() < 3 {
        return;
    }

    let triangles = triangulate_polygon(points);
    let base = vertices.len() as u32;
    let tint = color_to_f32(color);
    vertices.extend(points.iter().map(|point| SolidVertex {
        position: [point.x, point.y],
        color: tint,
    }));
    for [a, b, c] in triangles {
        indices.extend_from_slice(&[base + a as u32, base + b as u32, base + c as u32]);
    }
}

fn push_polyline(
    vertices: &mut Vec<SolidVertex>,
    indices: &mut Vec<u32>,
    points: &[Point],
    color: Color,
    width: f32,
    closed: bool,
) {
    let cleaned = prepare_polyline_points(points, closed);
    let closed = closed && cleaned.len() > 2;
    if cleaned.len() < 2 {
        return;
    }

    let tint = color_to_f32(color);
    let half = width * 0.5;
    let mut left_side = Vec::with_capacity(cleaned.len());
    let mut right_side = Vec::with_capacity(cleaned.len());

    for index in 0..cleaned.len() {
        let (left, right) = if !closed && index == 0 {
            let normal = match segment_normal(cleaned[0], cleaned[1]) {
                Some(normal) => normal,
                None => continue,
            };
            (
                offset_point(cleaned[0], normal.scale(half)),
                offset_point(cleaned[0], normal.scale(-half)),
            )
        } else if !closed && index == cleaned.len() - 1 {
            let normal = match segment_normal(cleaned[index - 1], cleaned[index]) {
                Some(normal) => normal,
                None => continue,
            };
            (
                offset_point(cleaned[index], normal.scale(half)),
                offset_point(cleaned[index], normal.scale(-half)),
            )
        } else {
            let prev_index = if index == 0 { cleaned.len() - 1 } else { index - 1 };
            let next_index = if index + 1 == cleaned.len() { 0 } else { index + 1 };
            (
                join_offset_point(
                    cleaned[prev_index],
                    cleaned[index],
                    cleaned[next_index],
                    half,
                    1.0,
                ),
                join_offset_point(
                    cleaned[prev_index],
                    cleaned[index],
                    cleaned[next_index],
                    half,
                    -1.0,
                ),
            )
        };
        left_side.push(left);
        right_side.push(right);
    }

    if left_side.len() < 2 || right_side.len() < 2 {
        return;
    }

    let base = vertices.len() as u32;
    for index in 0..left_side.len() {
        vertices.push(SolidVertex {
            position: [left_side[index].x, left_side[index].y],
            color: tint,
        });
        vertices.push(SolidVertex {
            position: [right_side[index].x, right_side[index].y],
            color: tint,
        });
    }

    let segments = if closed { left_side.len() } else { left_side.len() - 1 };
    for index in 0..segments {
        let next = (index + 1) % left_side.len();
        let current_left = base + (index as u32 * 2);
        let current_right = current_left + 1;
        let next_left = base + (next as u32 * 2);
        let next_right = next_left + 1;
        indices.extend_from_slice(&[
            current_left,
            current_right,
            next_right,
            current_left,
            next_right,
            next_left,
        ]);
    }
}

fn prepare_polyline_points(points: &[Point], closed: bool) -> Vec<Point> {
    let mut cleaned = Vec::with_capacity(points.len());
    for &point in points {
        if cleaned
            .last()
            .map(|last| squared_distance(*last, point) <= 0.0001)
            .unwrap_or(false)
        {
            continue;
        }
        cleaned.push(point);
    }

    if closed
        && cleaned.len() > 1
        && squared_distance(cleaned[0], *cleaned.last().unwrap()) <= 0.0001
    {
        cleaned.pop();
    }

    cleaned
}

fn join_offset_point(prev: Point, current: Point, next: Point, half: f32, side: f32) -> Point {
    let Some(prev_dir) = normalized_direction(prev, current) else {
        return current;
    };
    let Some(next_dir) = normalized_direction(current, next) else {
        return current;
    };

    let prev_normal = prev_dir.perpendicular().scale(side);
    let next_normal = next_dir.perpendicular().scale(side);
    let prev_origin = offset_point(current, prev_normal.scale(half));
    let next_origin = offset_point(current, next_normal.scale(half));

    if let Some(intersection) = line_intersection(
        prev_origin,
        prev_dir,
        next_origin,
        next_dir,
    ) {
        let offset = Vec2::from_points(current, intersection);
        let miter_limit = half * 4.0;
        if offset.length() <= miter_limit {
            return intersection;
        }
    }

    let average_normal = prev_normal.add(next_normal);
    if let Some(normalized) = average_normal.normalized() {
        return offset_point(current, normalized.scale(half));
    }

    offset_point(current, prev_normal.scale(half))
}

fn segment_normal(a: Point, b: Point) -> Option<Vec2> {
    normalized_direction(a, b).map(|direction| direction.perpendicular())
}

fn normalized_direction(a: Point, b: Point) -> Option<Vec2> {
    Vec2::from_points(a, b).normalized()
}

fn line_intersection(origin_a: Point, dir_a: Vec2, origin_b: Point, dir_b: Vec2) -> Option<Point> {
    let determinant = dir_a.cross(dir_b);
    if determinant.abs() <= 0.0001 {
        return None;
    }

    let delta = Vec2::from_points(origin_a, origin_b);
    let distance = delta.cross(dir_b) / determinant;
    Some(offset_point(origin_a, dir_a.scale(distance)))
}

fn offset_point(point: Point, offset: Vec2) -> Point {
    Point {
        x: point.x + offset.x,
        y: point.y + offset.y,
    }
}

fn squared_distance(a: Point, b: Point) -> f32 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    dx * dx + dy * dy
}

#[derive(Clone, Copy)]
struct Vec2 {
    x: f32,
    y: f32,
}

impl Vec2 {
    fn from_points(a: Point, b: Point) -> Self {
        Self {
            x: b.x - a.x,
            y: b.y - a.y,
        }
    }

    fn add(self, other: Self) -> Self {
        Self {
            x: self.x + other.x,
            y: self.y + other.y,
        }
    }

    fn scale(self, scalar: f32) -> Self {
        Self {
            x: self.x * scalar,
            y: self.y * scalar,
        }
    }

    fn perpendicular(self) -> Self {
        Self {
            x: -self.y,
            y: self.x,
        }
    }

    fn normalized(self) -> Option<Self> {
        let length = self.length();
        if length <= f32::EPSILON {
            return None;
        }
        Some(Self {
            x: self.x / length,
            y: self.y / length,
        })
    }

    fn cross(self, other: Self) -> f32 {
        self.x * other.y - self.y * other.x
    }

    fn length(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }
}

fn triangulate_polygon(points: &[Point]) -> Vec<[usize; 3]> {
    let count = points.len();
    if count < 3 {
        return Vec::new();
    }

    let signed_area: f32 = points.iter().enumerate().fold(0.0, |area, (index, point)| {
        let next = points[(index + 1) % count];
        area + (point.x * next.y - next.x * point.y)
    });
    let mut vertices: Vec<usize> = if signed_area >= 0.0 {
        (0..count).collect()
    } else {
        (0..count).rev().collect()
    };

    let mut triangles = Vec::with_capacity(count.saturating_sub(2));
    let mut guard = 0usize;
    while vertices.len() > 3 && guard < 10_000 {
        guard += 1;
        let len = vertices.len();
        let mut ear_found = false;
        for i in 0..len {
            let prev = vertices[(i + len - 1) % len];
            let current = vertices[i];
            let next = vertices[(i + 1) % len];
            let a = points[prev];
            let b = points[current];
            let c = points[next];

            if cross(a, b, c) <= 0.0 {
                continue;
            }

            let mut point_inside = false;
            for other in &vertices {
                if *other == prev || *other == current || *other == next {
                    continue;
                }
                if point_in_triangle(a, b, c, points[*other]) {
                    point_inside = true;
                    break;
                }
            }
            if point_inside {
                continue;
            }

            triangles.push([prev, current, next]);
            vertices.remove(i);
            ear_found = true;
            break;
        }

        if !ear_found {
            break;
        }
    }

    if vertices.len() == 3 {
        triangles.push([vertices[0], vertices[1], vertices[2]]);
    }
    triangles
}

fn cross(a: Point, b: Point, c: Point) -> f32 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

fn point_in_triangle(a: Point, b: Point, c: Point, p: Point) -> bool {
    let area = cross(a, b, c);
    if area.abs() <= f32::EPSILON {
        return false;
    }
    let s = cross(p, a, b);
    let t = cross(p, b, c);
    let u = cross(p, c, a);
    (s >= 0.0 && t >= 0.0 && u >= 0.0) || (s <= 0.0 && t <= 0.0 && u <= 0.0)
}

fn color_to_f32(color: Color) -> [f32; 4] {
    [
        color.r as f32 / 255.0,
        color.g as f32 / 255.0,
        color.b as f32 / 255.0,
        color.a as f32 / 255.0,
    ]
}

const SOLID_VERTEX_SHADER: &str = r#"#version 330 core
layout (location = 0) in vec2 a_position;
layout (location = 1) in vec4 a_color;

uniform vec2 u_screen_size;

out vec4 v_color;

void main() {
    vec2 clip = vec2(
        (a_position.x / u_screen_size.x) * 2.0 - 1.0,
        1.0 - (a_position.y / u_screen_size.y) * 2.0
    );
    gl_Position = vec4(clip, 0.0, 1.0);
    v_color = a_color;
}
"#;

const SOLID_FRAGMENT_SHADER: &str = r#"#version 330 core
in vec4 v_color;
out vec4 out_color;

void main() {
    out_color = v_color;
}
"#;

const TEXT_VERTEX_SHADER: &str = r#"#version 330 core
layout (location = 0) in vec2 a_position;
layout (location = 1) in vec2 a_uv;
layout (location = 2) in vec4 a_color;

uniform vec2 u_screen_size;

out vec2 v_uv;
out vec4 v_color;

void main() {
    vec2 clip = vec2(
        (a_position.x / u_screen_size.x) * 2.0 - 1.0,
        1.0 - (a_position.y / u_screen_size.y) * 2.0
    );
    gl_Position = vec4(clip, 0.0, 1.0);
    v_uv = a_uv;
    v_color = a_color;
}
"#;

const TEXT_FRAGMENT_SHADER: &str = r#"#version 330 core
in vec2 v_uv;
in vec4 v_color;

uniform sampler2D u_texture;

out vec4 out_color;

void main() {
    float alpha = texture(u_texture, v_uv).r;
    out_color = vec4(v_color.rgb, v_color.a * alpha);
}
"#;
