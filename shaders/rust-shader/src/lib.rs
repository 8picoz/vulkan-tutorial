#![cfg_attr(
    target_arch = "spirv",
    no_std,
    feature(register_attr),
    register_attr(spirv)
)]

#[cfg(not(target_arch = "spirv"))]
use spirv_std::macros::spirv;

use spirv_std::arch::IndexUnchecked;

//A/aが付いてるやつはSPIR-Vのアライメント考慮
use spirv_std::glam::{vec3a, vec4, Vec3A, Vec4};

#[spirv(vertex)]
pub fn main_vs(
    // gl_VertexIndex
    #[spirv(vertex_index)] vert_id: i32,
    // gl_Position
    #[spirv(position)] out_pos: &mut Vec4,
    // 何も指定せずに &mut したのでlayout(location = 0) outとなる
    color: &mut Vec3A,
) {
    *out_pos = *unsafe {
        [
            vec4(0.0, -1.0, 0.0, 1.0),
            vec4(1.0, 1.0, 0.0, 1.0),
            vec4(-1.0, 1.0, 0.0, 1.0),
        ]
        .index_unchecked(vert_id as usize)
    };

    *color = *unsafe {
        [
            vec3a(1.0, 0.0, 0.0),
            vec3a(0.0, 1.0, 0.0),
            vec3a(0.0, 0.0, 1.0),
        ]
        .index_unchecked(vert_id as usize)
    };
}

#[spirv(fragment)]
pub fn main_fs(
    // layout(location = 0) out
    output: &mut Vec4,
    // layout(location = 0) in
    color: Vec3A,
) {
    *output = color.extend(1.0);
}
