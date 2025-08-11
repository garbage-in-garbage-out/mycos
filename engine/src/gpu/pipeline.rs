//! GPU execution pipeline for Mycos.
//!
//! This module wires the WGSL kernels into a per tick command graph. The
//! sequence follows K1 → K2 → K3 → K4 → K5 (looped) → Kfinal as described in the
//! specification. The actual kernels live in `kernels.wgsl`; here we simply
//! issue dispatch commands in the proper order and insert barriers between
//! rounds.
//!
//! The implementation is intentionally minimal – it records commands but leaves
//! buffer management to the caller. The function will be compiled only when the
//! `webgpu` feature is enabled.

#![cfg(feature = "webgpu")]

use std::{convert::TryInto, sync::mpsc};
use wgpu::{
    BindGroup, Buffer, BufferDescriptor, BufferUsages, CommandEncoderDescriptor,
    ComputePassDescriptor, ComputePipeline, Device, Maintain, MapMode, Queue,
};

/// Convenience struct bundling all compute pipelines used during a tick.
///
/// The fields correspond to the WGSL entry points defined in `kernels.wgsl`.
#[allow(missing_docs)]
pub struct Pipelines {
    pub k1_detect_edges: ComputePipeline,
    pub k2_expand_count: ComputePipeline,
    pub k2_expand_emit: ComputePipeline,
    pub k3_resolve: ComputePipeline,
    pub k4_commit: ComputePipeline,
    pub k5_next_frontier: ComputePipeline,
    pub kfinal_finalize: ComputePipeline,
}

/// Execute one tick of the GPU pipeline.
///
/// `max_rounds` caps the number of wavefront rounds that may be executed. The
/// caller must provide the `frontier_counts` storage buffer bound at
/// `@group(0) @binding(10)`. The function will repeatedly dispatch K2–K5 rounds
/// until the frontier is empty or `max_rounds` is reached, then run
/// `Kfinal_finalize`.
///
/// Each round submits a command buffer and waits for completion so that the
/// frontier counts can be read back on the CPU. This makes the function
/// synchronous but keeps the loop logic simple and deterministic.
pub fn tick(
    device: &Device,
    queue: &Queue,
    bind_group: &BindGroup,
    pipelines: &Pipelines,
    frontier_counts: &Buffer,
    max_rounds: u32,
) {
    const FRONTIER_SIZE: u64 = std::mem::size_of::<[u32; 4]>() as u64;

    let readback = device.create_buffer(&BufferDescriptor {
        label: Some("frontier-counts-readback"),
        size: FRONTIER_SIZE,
        usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    // Helper to copy frontier counts to `readback` and return whether the
    // frontier is empty.
    let mut fetch_empty = |mut encoder: wgpu::CommandEncoder| -> bool {
        encoder.copy_buffer_to_buffer(frontier_counts, 0, &readback, 0, FRONTIER_SIZE);
        queue.submit(Some(encoder.finish()));

        let slice = readback.slice(..);
        let (sender, receiver) = mpsc::channel();
        slice.map_async(MapMode::Read, move |v| sender.send(v).unwrap());
        device.poll(Maintain::Wait);
        receiver.recv().unwrap().unwrap();
        let data = slice.get_mapped_range();
        let on = u32::from_le_bytes(data[0..4].try_into().unwrap());
        let off = u32::from_le_bytes(data[4..8].try_into().unwrap());
        let toggle = u32::from_le_bytes(data[8..12].try_into().unwrap());
        drop(data);
        readback.unmap();
        on == 0 && off == 0 && toggle == 0
    };

    // K1: detect edges and seed the frontier.
    {
        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("K1_detect_edges"),
        });
        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("K1_detect_edges"),
                ..Default::default()
            });
            pass.set_pipeline(&pipelines.k1_detect_edges);
            pass.set_bind_group(0, bind_group, &[]);
            pass.dispatch_workgroups(1, 1, 1);
        }

        if fetch_empty(encoder) {
            // Frontier empty after seeding; no rounds to execute.
            let mut final_encoder = device.create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Kfinal_finalize"),
            });
            let mut pass = final_encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("Kfinal_finalize"),
                ..Default::default()
            });
            pass.set_pipeline(&pipelines.kfinal_finalize);
            pass.set_bind_group(0, bind_group, &[]);
            pass.dispatch_workgroups(1, 1, 1);
            queue.submit(Some(final_encoder.finish()));
            return;
        }
    }

    // Wavefront micro-step loop.
    let mut round = 0;
    while round < max_rounds {
        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("mycos-round"),
        });
        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("K2_expand_count"),
                ..Default::default()
            });
            pass.set_pipeline(&pipelines.k2_expand_count);
            pass.set_bind_group(0, bind_group, &[]);
            pass.dispatch_workgroups(1, 1, 1);
        }
        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("K2_expand_emit"),
                ..Default::default()
            });
            pass.set_pipeline(&pipelines.k2_expand_emit);
            pass.set_bind_group(0, bind_group, &[]);
            pass.dispatch_workgroups(1, 1, 1);
        }
        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("K3_resolve"),
                ..Default::default()
            });
            pass.set_pipeline(&pipelines.k3_resolve);
            pass.set_bind_group(0, bind_group, &[]);
            pass.dispatch_workgroups(1, 1, 1);
        }
        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("K4_commit"),
                ..Default::default()
            });
            pass.set_pipeline(&pipelines.k4_commit);
            pass.set_bind_group(0, bind_group, &[]);
            pass.dispatch_workgroups(1, 1, 1);
        }
        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("K5_next_frontier"),
                ..Default::default()
            });
            pass.set_pipeline(&pipelines.k5_next_frontier);
            pass.set_bind_group(0, bind_group, &[]);
            pass.dispatch_workgroups(1, 1, 1);
        }

        if fetch_empty(encoder) {
            break;
        }
        round += 1;
    }

    // Finalize tick by copying Curr→Prev and writing metrics.
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("Kfinal_finalize"),
    });
    {
        let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("Kfinal_finalize"),
            ..Default::default()
        });
        pass.set_pipeline(&pipelines.kfinal_finalize);
        pass.set_bind_group(0, bind_group, &[]);
        pass.dispatch_workgroups(1, 1, 1);
    }

    queue.submit(Some(encoder.finish()));
}
