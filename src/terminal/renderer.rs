use std::mem::size_of;

use bytemuck::{Pod, Zeroable};
use eframe::{
    CreationContext,
    egui::{self, Pos2, Rect, Vec2},
    egui_wgpu::{self, wgpu},
};

use super::{Rgb, TerminalGrid, grid::DEFAULT_BACKGROUND};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    position: [f32; 2],
    color: [f32; 4],
}

struct RenderResources {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    vertex_capacity: u64,
}

impl RenderResources {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ripple-terminal-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("terminal.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ripple-terminal-pipeline-layout"),
            bind_group_layouts: &[],
            immediate_size: 0,
        });
        let attributes = [
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: size_of::<[f32; 2]>() as u64,
                shader_location: 1,
            },
        ];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ripple-terminal-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &attributes,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let vertex_capacity = 4 * size_of::<Vertex>() as u64;
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ripple-terminal-vertices"),
            size: vertex_capacity,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            pipeline,
            vertex_buffer,
            vertex_capacity,
        }
    }

    fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, vertices: &[Vertex]) {
        if vertices.is_empty() {
            return;
        }
        let required = std::mem::size_of_val(vertices) as u64;
        if required > self.vertex_capacity {
            self.vertex_capacity = required.next_power_of_two();
            self.vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("ripple-terminal-vertices"),
                size: self.vertex_capacity,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(vertices));
    }

    fn paint(&self, render_pass: &mut wgpu::RenderPass<'static>, vertex_count: u32) {
        if vertex_count == 0 {
            return;
        }
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.draw(0..vertex_count, 0..1);
    }
}

pub struct TerminalGpuRenderer;

impl TerminalGpuRenderer {
    pub fn install(cc: &CreationContext<'_>) -> Result<(), String> {
        let render_state = cc
            .wgpu_render_state
            .as_ref()
            .ok_or_else(|| "The eframe wgpu renderer is unavailable".to_owned())?;
        let resources = RenderResources::new(&render_state.device, render_state.target_format);
        render_state
            .renderer
            .write()
            .callback_resources
            .insert(resources);
        Ok(())
    }
}

struct TerminalPaintCallback {
    vertices: Vec<Vertex>,
}

impl egui_wgpu::CallbackTrait for TerminalPaintCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if let Some(resources) = resources.get_mut::<RenderResources>() {
            resources.upload(device, queue, &self.vertices);
        }
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        resources: &egui_wgpu::CallbackResources,
    ) {
        if let Some(resources) = resources.get::<RenderResources>() {
            resources.paint(render_pass, self.vertices.len() as u32);
        }
    }
}

pub fn gpu_cell_paint_callback(
    viewport: Rect,
    grid: &TerminalGrid,
    cell_width: f32,
    cell_height: f32,
    origin: Vec2,
    scroll_offset: usize,
    selection: Option<((usize, usize), (usize, usize))>,
) -> egui::epaint::PaintCallback {
    let mut vertices = Vec::new();
    for row_index in 0..grid.dimensions().1 {
        let row = grid.visible_line(row_index, scroll_offset);
        for (column_index, cell) in row.iter().take(grid.dimensions().0).enumerate() {
            let selected = selection.is_some_and(|(start, end)| {
                let (start, end) = ordered_positions(start, end);
                row_index >= start.0
                    && row_index <= end.0
                    && (row_index != start.0 || column_index >= start.1)
                    && (row_index != end.0 || column_index <= end.1)
            });
            let background = if selected {
                Rgb(65, 65, 65)
            } else {
                cell.background
            };
            if background == DEFAULT_BACKGROUND {
                continue;
            }
            let min = Pos2::new(
                viewport.left() + origin.x + column_index as f32 * cell_width,
                viewport.top() + origin.y + row_index as f32 * cell_height,
            );
            let max = min + Vec2::new(cell_width, cell_height);
            push_quad(&mut vertices, viewport, min, max, background);
        }
    }

    if scroll_offset == 0 && grid.cursor_visible {
        let (cursor_column, cursor_row) = grid.cursor();
        let cursor_min = Pos2::new(
            viewport.left() + origin.x + cursor_column as f32 * cell_width,
            viewport.top() + origin.y + cursor_row as f32 * cell_height,
        );
        push_quad(
            &mut vertices,
            viewport,
            cursor_min,
            cursor_min + Vec2::new(2.0, cell_height - 2.0),
            Rgb(210, 210, 210),
        );
    }

    egui_wgpu::Callback::new_paint_callback(viewport, TerminalPaintCallback { vertices })
}

fn ordered_positions(
    first: (usize, usize),
    second: (usize, usize),
) -> ((usize, usize), (usize, usize)) {
    if first <= second {
        (first, second)
    } else {
        (second, first)
    }
}

fn push_quad(vertices: &mut Vec<Vertex>, viewport: Rect, min: Pos2, max: Pos2, color: Rgb) {
    if viewport.width() <= 0.0 || viewport.height() <= 0.0 {
        return;
    }
    let left = ((min.x - viewport.left()) / viewport.width()) * 2.0 - 1.0;
    let right = ((max.x - viewport.left()) / viewport.width()) * 2.0 - 1.0;
    let top = 1.0 - ((min.y - viewport.top()) / viewport.height()) * 2.0;
    let bottom = 1.0 - ((max.y - viewport.top()) / viewport.height()) * 2.0;
    let color = [
        color.0 as f32 / 255.0,
        color.1 as f32 / 255.0,
        color.2 as f32 / 255.0,
        1.0,
    ];
    let a = Vertex {
        position: [left, top],
        color,
    };
    let b = Vertex {
        position: [right, top],
        color,
    };
    let c = Vertex {
        position: [right, bottom],
        color,
    };
    let d = Vertex {
        position: [left, bottom],
        color,
    };
    vertices.extend_from_slice(&[a, b, c, a, c, d]);
}
