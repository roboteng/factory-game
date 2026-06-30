//! Headless render + PNG snapshot harness for the `views` screen UIs.
//!
//! Adapted from Bevy 0.19's official `examples/app/headless_renderer.rs`: a camera
//! renders to an offscreen `Image` (`COPY_SRC`), a render-graph system copies that GPU
//! texture into a mappable buffer, and `receive_image_from_buffer` maps it back to the
//! main world over a channel. Unlike the example we don't save to disk inside the render
//! loop or exit the app — [`render_scene`] drives a fixed number of frames and returns the
//! captured pixels as an [`image::RgbaImage`].
//!
//! ## Adding snapshots
//! Each case is one [`snapshot`] call with a unique `name`. To add a new screen, or a new
//! variant of an existing screen, add another `snapshot(...)` call — there is no per-screen
//! boilerplate. Use `screen__variant` names (e.g. `"hotbar__placing_rock"`) so reference
//! files group together under `tests/snapshots/`.
//!
//! ## Workflow
//! - `UPDATE_SNAPSHOTS=1 cargo test -p views` writes/overwrites reference PNGs (the
//!   `cargo-insta`-style accept step). Inspect them, then commit.
//! - `cargo test -p views` compares against the committed references; on mismatch it writes
//!   `<name>.actual.png` beside the reference and fails.

#![allow(dead_code)] // helpers are used selectively across test files

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use bevy::{
    camera::RenderTarget,
    prelude::*,
    render::{
        render_asset::RenderAssets,
        render_resource::{
            Buffer, BufferDescriptor, BufferUsages, CommandEncoderDescriptor, Extent3d, MapMode,
            PollType, TexelCopyBufferInfo, TexelCopyBufferLayout, TextureFormat, TextureUsages,
        },
        renderer::{RenderContext, RenderDevice, RenderGraph, RenderQueue},
        texture::GpuImage,
        Extract, Render, RenderApp, RenderSystems,
    },
    ui::IsDefaultUiCamera,
    window::ExitCondition,
    winit::WinitPlugin,
};
use crossbeam_channel::{Receiver, Sender};
use image::RgbaImage;

/// Default canvas size. Fixed so the views' percent-based layouts resolve deterministically.
pub const DEFAULT_SIZE: UVec2 = UVec2::new(1280, 720);

/// Render the scene spawned by `spawn` at [`DEFAULT_SIZE`] and assert it matches the
/// committed reference named `name`. This is the one-call entry point for a snapshot case.
pub fn snapshot(name: &str, spawn: impl FnOnce(&mut Commands) + Send + Sync + 'static) {
    let actual = render_scene(DEFAULT_SIZE, spawn);
    assert_image_snapshot(name, actual);
}

// ---------------------------------------------------------------------------
// Headless render harness
// ---------------------------------------------------------------------------

/// How many frames to render before reading back pixels. Generous to cover async render
/// device init and UI/pipeline warmup (the Bevy example pre-rolls 40).
const PRE_ROLL_FRAMES: usize = 60;

#[derive(Resource)]
struct RenderSize(Extent3d);

#[derive(Resource)]
struct SpawnFn(Option<Box<dyn FnOnce(&mut Commands) + Send + Sync>>);

/// Build a headless app, render `spawn`'s scene to an offscreen image, and return the pixels.
pub fn render_scene(
    size: UVec2,
    spawn: impl FnOnce(&mut Commands) + Send + Sync + 'static,
) -> RgbaImage {
    let extent = Extent3d {
        width: size.x,
        height: size.y,
        ..default()
    };

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .build()
            // Winit owns an event loop that must run on the main thread; `cargo test`
            // runs each test on a worker thread, so winit would panic on macOS. We drive
            // frames manually with `app.update()` instead.
            .disable::<WinitPlugin>()
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                ..default()
            })
            .set(AssetPlugin {
                file_path: "../assets/".to_string(),
                ..default()
            })
            // Match the examples: drop heavy/non-deterministic plugins (AA off in particular
            // keeps pixel output stable).
            .disable::<bevy::pbr::PbrPlugin>()
            .disable::<bevy::gltf::GltfPlugin>()
            .disable::<bevy::gilrs::GilrsPlugin>()
            .disable::<bevy::animation::AnimationPlugin>()
            .disable::<bevy::gizmos::GizmoPlugin>()
            .disable::<bevy::gizmos_render::GizmoRenderPlugin>()
            .disable::<bevy::light::LightPlugin>()
            .disable::<bevy::anti_alias::AntiAliasPlugin>()
            .disable::<bevy::post_process::PostProcessPlugin>(),
    )
    .add_plugins(ImageCopyPlugin)
    .insert_resource(RenderSize(extent))
    .insert_resource(SpawnFn(Some(Box::new(spawn))))
    .add_systems(Startup, setup);

    // Initialize plugins (creates the render device, inserted into the main world) before
    // we start ticking frames.
    app.finish();
    app.cleanup();
    for _ in 0..PRE_ROLL_FRAMES {
        app.update();
    }

    // Drain the channel and keep the most recent frame.
    let receiver = app.world().resource::<MainWorldReceiver>();
    let mut data = Vec::new();
    while let Ok(latest) = receiver.try_recv() {
        data = latest;
    }
    assert!(
        !data.is_empty(),
        "no rendered frame was captured after {PRE_ROLL_FRAMES} frames \
         (is a GPU/wgpu adapter available?)"
    );

    strip_row_padding(data, size.x, size.y)
}

/// `setup` runs once in `Startup`, after `app.finish()` has inserted `RenderDevice` into the
/// main world. It creates the offscreen target, the copier, the camera, then the user scene.
fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    render_device: Res<RenderDevice>,
    size: Res<RenderSize>,
    mut spawn_fn: ResMut<SpawnFn>,
) {
    let extent = size.0;
    let mut target = Image::new_target_texture(
        extent.width,
        extent.height,
        TextureFormat::Rgba8UnormSrgb,
        None,
    );
    target.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let handle = images.add(target);

    commands.spawn(ImageCopier::new(handle.clone(), extent, &render_device));
    // In Bevy 0.19 the render target is a standalone component, not a `Camera` field.
    // `IsDefaultUiCamera` is essential: untargeted UI lays out/renders against the default
    // UI camera (normally the window's). Headless has no window camera, so without this the
    // UI root resolves to zero size and nothing draws.
    commands.spawn((
        Camera2d,
        RenderTarget::Image(handle.into()),
        IsDefaultUiCamera,
    ));

    if let Some(spawn) = spawn_fn.0.take() {
        spawn(&mut commands);
    }
}

/// GPU rows are padded to a 256-byte alignment; drop the padding to get tight RGBA8 pixels.
fn strip_row_padding(data: Vec<u8>, width: u32, height: u32) -> RgbaImage {
    let row_bytes = width as usize * 4;
    let padded = RenderDevice::align_copy_bytes_per_row(row_bytes);
    let pixels: Vec<u8> = if row_bytes == padded {
        data
    } else {
        data.chunks(padded)
            .take(height as usize)
            .flat_map(|row| &row[..row_bytes])
            .copied()
            .collect()
    };
    RgbaImage::from_raw(width, height, pixels).expect("pixel buffer did not match image size")
}

// --- ImageCopy plumbing (verbatim structure from Bevy's headless_renderer example) --------

#[derive(Resource, Deref)]
struct MainWorldReceiver(Receiver<Vec<u8>>);

#[derive(Resource, Deref)]
struct RenderWorldSender(Sender<Vec<u8>>);

struct ImageCopyPlugin;
impl Plugin for ImageCopyPlugin {
    fn build(&self, app: &mut App) {
        let (s, r) = crossbeam_channel::unbounded();
        let render_app = app
            .insert_resource(MainWorldReceiver(r))
            .sub_app_mut(RenderApp);
        render_app
            .insert_resource(RenderWorldSender(s))
            .add_systems(ExtractSchedule, image_copy_extract)
            .add_systems(
                Render,
                receive_image_from_buffer.after(RenderSystems::Render),
            )
            .add_systems(RenderGraph, image_copy_driver);
    }
}

#[derive(Clone, Component)]
struct ImageCopier {
    buffer: Buffer,
    enabled: Arc<AtomicBool>,
    src_image: Handle<Image>,
}

impl ImageCopier {
    fn new(src_image: Handle<Image>, size: Extent3d, render_device: &RenderDevice) -> ImageCopier {
        let padded_bytes_per_row = RenderDevice::align_copy_bytes_per_row(size.width as usize * 4);
        let cpu_buffer = render_device.create_buffer(&BufferDescriptor {
            label: None,
            size: padded_bytes_per_row as u64 * size.height as u64,
            usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        ImageCopier {
            buffer: cpu_buffer,
            src_image,
            enabled: Arc::new(AtomicBool::new(true)),
        }
    }

    fn enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }
}

#[derive(Clone, Default, Resource, Deref, DerefMut)]
struct ImageCopiers(pub Vec<ImageCopier>);

fn image_copy_extract(mut commands: Commands, image_copiers: Extract<Query<&ImageCopier>>) {
    commands.insert_resource(ImageCopiers(
        image_copiers.iter().cloned().collect::<Vec<ImageCopier>>(),
    ));
}

fn image_copy_driver(
    render_context: RenderContext,
    image_copiers: Res<ImageCopiers>,
    render_queue: Res<RenderQueue>,
    gpu_images: Res<RenderAssets<GpuImage>>,
) {
    for image_copier in image_copiers.iter() {
        if !image_copier.enabled() {
            continue;
        }

        let src_image = gpu_images.get(&image_copier.src_image).unwrap();
        let mut encoder = render_context
            .render_device()
            .create_command_encoder(&CommandEncoderDescriptor::default());

        let block_dimensions = src_image.texture_descriptor.format.block_dimensions();
        let block_size = src_image
            .texture_descriptor
            .format
            .block_copy_size(None)
            .unwrap();

        let padded_bytes_per_row = RenderDevice::align_copy_bytes_per_row(
            (src_image.texture_descriptor.size.width as usize / block_dimensions.0 as usize)
                * block_size as usize,
        );

        encoder.copy_texture_to_buffer(
            src_image.texture.as_image_copy(),
            TexelCopyBufferInfo {
                buffer: &image_copier.buffer,
                layout: TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(
                        std::num::NonZero::<u32>::new(padded_bytes_per_row as u32)
                            .unwrap()
                            .into(),
                    ),
                    rows_per_image: None,
                },
            },
            src_image.texture_descriptor.size,
        );

        render_queue.submit(std::iter::once(encoder.finish()));
    }
}

fn receive_image_from_buffer(
    image_copiers: Res<ImageCopiers>,
    render_device: Res<RenderDevice>,
    sender: Res<RenderWorldSender>,
) {
    for image_copier in image_copiers.0.iter() {
        if !image_copier.enabled() {
            continue;
        }

        let buffer_slice = image_copier.buffer.slice(..);
        let (s, r) = crossbeam_channel::bounded(1);

        buffer_slice.map_async(MapMode::Read, move |r| match r {
            Ok(r) => s.send(r).expect("Failed to send map update"),
            Err(err) => panic!("Failed to map buffer {err}"),
        });

        render_device
            .poll(PollType::wait_indefinitely())
            .expect("Failed to poll device for map async");

        r.recv().expect("Failed to receive the map_async message");

        let _ = sender.send(buffer_slice.get_mapped_range().to_vec());

        image_copier.buffer.unmap();
    }
}

// ---------------------------------------------------------------------------
// Snapshot comparison
// ---------------------------------------------------------------------------

/// Per-channel difference (0-255) at or below which two pixels are considered equal. Absorbs
/// trivial rasterization jitter.
const CHANNEL_TOLERANCE: u8 = 8;
/// Maximum fraction of pixels allowed to differ before the snapshot is considered a mismatch.
const MAX_DIFF_FRACTION: f64 = 0.002;

/// Compare `actual` against the committed reference `tests/snapshots/<name>.png`.
///
/// Writes (and passes) when the reference is missing or `UPDATE_SNAPSHOTS` is set; otherwise
/// asserts the images match within tolerance, dumping `<name>.actual.png` on failure.
pub fn assert_image_snapshot(name: &str, actual: RgbaImage) {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots");
    let reference_path = dir.join(format!("{name}.png"));

    let updating = std::env::var_os("UPDATE_SNAPSHOTS").is_some_and(|v| v != "0" && v != "");
    if updating || !reference_path.exists() {
        std::fs::create_dir_all(&dir).expect("failed to create snapshots dir");
        actual
            .save(&reference_path)
            .expect("failed to write reference png");
        eprintln!(
            "snapshot `{name}`: wrote reference {}",
            reference_path.display()
        );
        return;
    }

    let reference = image::open(&reference_path)
        .unwrap_or_else(|e| panic!("failed to read reference `{name}`: {e}"))
        .to_rgba8();

    assert_eq!(
        (actual.width(), actual.height()),
        (reference.width(), reference.height()),
        "snapshot `{name}`: size {:?} != reference size {:?}",
        (actual.width(), actual.height()),
        (reference.width(), reference.height()),
    );

    // Build a diff image as we count: differing pixels are painted magenta; matching pixels
    // become a dimmed grayscale of the reference so the UI stays visible for context.
    let mut differing = 0u64;
    let mut diff = RgbaImage::new(actual.width(), actual.height());
    for ((a, b), out) in actual
        .pixels()
        .zip(reference.pixels())
        .zip(diff.pixels_mut())
    {
        let differs =
            a.0.iter()
                .zip(b.0.iter())
                .any(|(x, y)| x.abs_diff(*y) > CHANNEL_TOLERANCE);
        if differs {
            differing += 1;
            *out = image::Rgba([255, 0, 255, 255]);
        } else {
            // Rec. 601 luma of the reference, darkened so magenta pops.
            let [r, g, bl, _] = b.0;
            let luma = (0.299 * r as f32 + 0.587 * g as f32 + 0.114 * bl as f32) as u8 / 3;
            *out = image::Rgba([luma, luma, luma, 255]);
        }
    }

    let total = (actual.width() * actual.height()) as f64;
    let fraction = differing as f64 / total;

    if fraction > MAX_DIFF_FRACTION {
        let actual_path = dir.join(format!("{name}.actual.png"));
        let diff_path = dir.join(format!("{name}.diff.png"));
        actual
            .save(&actual_path)
            .expect("failed to write actual png");
        diff.save(&diff_path).expect("failed to write diff png");
        panic!(
            "snapshot `{name}` mismatch: {differing} px differ ({:.4}% > {:.4}% allowed).\n\
             wrote actual: {}\n\
             wrote diff (magenta = changed): {}\n\
             If this change is intentional, rerun with UPDATE_SNAPSHOTS=1 to accept.",
            fraction * 100.0,
            MAX_DIFF_FRACTION * 100.0,
            actual_path.display(),
            diff_path.display(),
        );
    }
}
