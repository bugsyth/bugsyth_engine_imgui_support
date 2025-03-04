use bugsyth_engine::prelude::*;

#[derive(Clone, Copy)]
struct Vertex {
    position: [f32; 2],
    color: [f32; 3],
}
implement_vertex!(Vertex, position, color);

fn main() -> EngineResult {
    let (event_loop, mut ctx) = init("imgui", (960, 720))?;
    ctx.new_program(
        "3d",
        "
    in vec2 position;
    in vec3 color;

    out vec3 v_color;

    uniform mat4 persp;
    uniform mat4 view;
    uniform mat4 matrix;

    void main() {
        mat4 modelview = view * matrix;
        v_color = color;
        gl_Position = persp * modelview * vec4(position, 0.0, 1.0);
    }
    ",
        "
    in vec3 v_color;

    out vec4 color;

    void main() {
        color = vec4(v_color, 1.0);
    }
    ",
        None,
    )
    .unwrap();
    let imgui = bugsyth_engine_imgui_support::init(&ctx.window, &ctx.display, |_, _, _| {});
    let game = Game {
        tri: Triangle {
            vbo: VertexBuffer::new(
                &ctx.display,
                &[
                    Vertex {
                        position: [-0.5, -0.5],
                        color: [1.0, 0.0, 0.0],
                    },
                    Vertex {
                        position: [0.5, 0.5],
                        color: [0.0, 1.0, 0.0],
                    },
                    Vertex {
                        position: [-0.5, 0.5],
                        color: [0.0, 0.0, 1.0],
                    },
                ],
            )
            .unwrap(),
            ibo: NoIndices(PrimitiveType::TrianglesList),
            draw_params: DrawParameters {
                ..Default::default()
            },
        },
        pos: Vec3::zero(),
        imgui,
    };
    run(game, event_loop, ctx)?;
    Ok(())
}

struct Game {
    tri: Triangle<'static>,
    pos: Vec3<f32>,
    imgui: bugsyth_engine_imgui_support::ImGui,
}

impl GameState for Game {
    fn update(&mut self, ctx: &mut Context) {
        self.imgui.update_dt(ctx.dt);
        bugsyth_engine::context::camera::CameraState::free_cam(ctx.dt, ctx, 1.0, 1.0);
    }
    fn draw(&mut self, ctx: &mut Context, renderer: &mut impl Renderer) {
        renderer.clear_color(0.0, 0.0, 0.0, 1.0);
        renderer
            .draw(
                ctx,
                &self.tri,
                &uniform! {
                    persp: ctx.camera.get_perspective(),
                    view: ctx.camera.get_view(),
                    matrix: math_helper::mat4_as_array(Mat4::translation_3d(self.pos)),
                },
            )
            .unwrap();

        self.imgui
            .platform
            .prepare_frame(self.imgui.context.io_mut(), &ctx.window)
            .unwrap();
        let ui = self.imgui.context.frame();

        ui.window("imgui")
            .size(
                [250.0, 150.0],
                bugsyth_engine_imgui_support::Condition::FirstUseEver,
            )
            .build(|| {
                ui.text_wrapped("Position:");

                ui.slider("x", -1.0, 1.0, &mut self.pos.x);
                ui.slider("y", -1.0, 1.0, &mut self.pos.y);
                ui.slider("z", -1.0, 1.0, &mut self.pos.z);

                ui.separator();
                ui.text(format!("FPS: {}", (1.0 / ctx.dt).floor()));
            });

        self.imgui.platform.prepare_render(&ui, &ctx.window);
        let draw_data = self.imgui.context.render();
        self.imgui
            .renderer
            .render(renderer.get_surface_mut(), draw_data)
            .unwrap();
    }
    fn event(&mut self, ctx: &mut Context, event: &WindowEvent) {
        self.imgui.event(&ctx.window, event);
    }
}

struct Triangle<'a> {
    vbo: VertexBuffer<Vertex>,
    ibo: NoIndices,
    draw_params: DrawParameters<'a>,
}

impl<'a> Drawable for Triangle<'a> {
    fn get_vbo(&self) -> impl MultiVerticesSource {
        &self.vbo
    }
    fn get_ibo(&self) -> impl Into<IndicesSource> {
        &self.ibo
    }
    fn get_program(&self) -> String {
        "3d".to_string()
    }
    fn get_draw_params(&self) -> DrawParameters {
        self.draw_params.clone()
    }
}
