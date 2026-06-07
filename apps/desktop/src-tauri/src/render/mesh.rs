//! Procedural mesh primitives: plane, box, sphere.

use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

pub struct MeshData {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

/// Unit plane in the XY plane (facing +Z), 1×1, centered. Scaled later.
pub fn plane() -> MeshData {
    let n = [0.0, 0.0, 1.0];
    let vertices = vec![
        Vertex { pos: [-0.5, -0.5, 0.0], normal: n, uv: [0.0, 1.0] },
        Vertex { pos: [ 0.5, -0.5, 0.0], normal: n, uv: [1.0, 1.0] },
        Vertex { pos: [ 0.5,  0.5, 0.0], normal: n, uv: [1.0, 0.0] },
        Vertex { pos: [-0.5,  0.5, 0.0], normal: n, uv: [0.0, 0.0] },
    ];
    let indices = vec![0, 1, 2, 0, 2, 3];
    MeshData { vertices, indices }
}

/// Unit cube centered at origin, 1×1×1.
pub fn cube() -> MeshData {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // (normal, [4 corner positions ccw])
    let faces: [( [f32;3], [[f32;3];4] ); 6] = [
        ([0.0, 0.0, 1.0],  [[-0.5,-0.5,0.5],[0.5,-0.5,0.5],[0.5,0.5,0.5],[-0.5,0.5,0.5]]),     // +Z
        ([0.0, 0.0,-1.0],  [[0.5,-0.5,-0.5],[-0.5,-0.5,-0.5],[-0.5,0.5,-0.5],[0.5,0.5,-0.5]]), // -Z
        ([1.0, 0.0, 0.0],  [[0.5,-0.5,0.5],[0.5,-0.5,-0.5],[0.5,0.5,-0.5],[0.5,0.5,0.5]]),     // +X
        ([-1.0,0.0, 0.0],  [[-0.5,-0.5,-0.5],[-0.5,-0.5,0.5],[-0.5,0.5,0.5],[-0.5,0.5,-0.5]]), // -X
        ([0.0, 1.0, 0.0],  [[-0.5,0.5,0.5],[0.5,0.5,0.5],[0.5,0.5,-0.5],[-0.5,0.5,-0.5]]),     // +Y
        ([0.0,-1.0, 0.0],  [[-0.5,-0.5,-0.5],[0.5,-0.5,-0.5],[0.5,-0.5,0.5],[-0.5,-0.5,0.5]]), // -Y
    ];

    let uvs = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];

    for (normal, corners) in faces {
        let base = vertices.len() as u32;
        for (i, c) in corners.iter().enumerate() {
            vertices.push(Vertex { pos: *c, normal, uv: uvs[i] });
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    MeshData { vertices, indices }
}

/// UV sphere, radius 0.5.
pub fn sphere(segments: u32, rings: u32) -> MeshData {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let r = 0.5;

    for ring in 0..=rings {
        let phi = std::f32::consts::PI * ring as f32 / rings as f32;
        let (sp, cp) = phi.sin_cos();
        for seg in 0..=segments {
            let theta = 2.0 * std::f32::consts::PI * seg as f32 / segments as f32;
            let (st, ct) = theta.sin_cos();
            let normal = [sp * ct, cp, sp * st];
            vertices.push(Vertex {
                pos: [r * normal[0], r * normal[1], r * normal[2]],
                normal,
                uv: [seg as f32 / segments as f32, ring as f32 / rings as f32],
            });
        }
    }

    let stride = segments + 1;
    for ring in 0..rings {
        for seg in 0..segments {
            let a = ring * stride + seg;
            let b = a + stride;
            indices.extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
        }
    }
    MeshData { vertices, indices }
}
