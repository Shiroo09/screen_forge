//! Recording status overlay: a small always-on-top bar showing elapsed time,
//! frame rate and memory, with pause / restart / stop. Port of
//! `ScreenForge/Windows/RecordingOverlayWindow.cs` (the control bar half).
//!
//! It is a second egui viewport rather than a Win32 window: the app already owns
//! the egui event loop, so the bar gets fonts and input for free and shares the
//! live [`sf_record::PauseHandle`] with the panel instead of opening a channel.
//!
//! shortcut: the bar is a normal topmost window, not `WDA_EXCLUDEFROMCAPTURE`.
//! Position it away from the recorded region if it must not appear in the GIF.

use eframe::egui;
use sf_record::{PauseHandle, RecordPhase};

use crate::theme;

/// Where the bar sits and what it covers, in virtual-screen coordinates.
#[derive(Clone)]
pub struct OverlaySpec {
    /// Region being recorded.
    pub region: (i32, i32, i32, i32),
    /// Top-left corner of the whole virtual screen.
    pub origin: (i32, i32),
    pub handle: PauseHandle,
    /// Whether the GIF encoder is in use (drives the memory readout).
    pub gif: bool,
}

/// egui window bar size. Keep in sync with `BAR_SIZE` usage in `ui`.
const BAR_SIZE: [f32; 2] = [330.0, 46.0];

/// Ctrl+Shift+P toggles pause, matching the C# overlay's hotkey
/// (`RecordingOverlayWindow.cs:413`).
fn pause_shortcut() -> egui::KeyboardShortcut {
    egui::KeyboardShortcut::new(
        egui::Modifiers { ctrl: true, shift: true, ..Default::default() },
        egui::Key::P,
    )
}

/// Paints the bar. Call once per pass while a recording is running.
pub fn ui(ctx: &egui::Context, spec: &OverlaySpec) {
    let id = egui::ViewportId::from_hash_of("screenforge-recording-bar");
    let (vx, vy) = spec.origin;
    // Park the bar just above the recorded region so it hides as little as
    // possible; clamp into the virtual screen so it is never off-screen.
    let x = spec.region.0.max(vx + 8) as f32;
    let y = (spec.region.1 - BAR_SIZE[1] as i32 - 6).max(vy + 8) as f32;

    let builder = egui::ViewportBuilder::default()
        .with_title("ScreenForge — Kayıt")
        .with_inner_size(BAR_SIZE)
        .with_min_inner_size(BAR_SIZE)
        .with_resizable(false)
        .with_decorations(false)
        .with_always_on_top()
        .with_mouse_passthrough(false)
        .with_position([x, y]);

    let session = spec.handle.session().clone();
    let handle = spec.handle.clone();
    let gif = spec.gif;
    let shortcut = pause_shortcut();

    ctx.show_viewport_deferred(id, builder, move |ctx, _class| {
        let frame = egui::Frame::new()
            .fill(theme::SURFACE)
            .stroke(egui::Stroke::new(1.0, theme::ACCENT))
            .corner_radius(8.0)
            .inner_margin(egui::Margin::symmetric(10, 6));

        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
            ui.horizontal(|ui| {
                if session.is_paused() {
                    ui.label(egui::RichText::new("||").color(theme::TEXT_DIM).strong());
                } else {
                    ui.label(egui::RichText::new("●").color(theme::ACCENT).size(16.0));
                }

                let elapsed = session.elapsed_ms() / 1000;
                ui.label(
                    egui::RichText::new(format!("{:02}:{:02}", elapsed / 60, elapsed % 60))
                        .color(theme::TEXT)
                        .strong(),
                );

                // Measured rate first; before the first second it reads the target.
                let fps = session.recorded_fps();
                let fps = if fps > 0.0 { fps } else { session.target_fps() as f64 };
                ui.label(egui::RichText::new(format!("{fps:.0} fps")).color(theme::TEXT_DIM));

                if gif {
                    let mb = session.bytes() as f64 / (1024.0 * 1024.0);
                    ui.label(egui::RichText::new(format!("{mb:.1} MB")).color(theme::TEXT_DIM));
                }
            });

            ui.add_space(2.0);
            ui.horizontal(|ui| {
                let label = if session.is_paused() { "Devam et" } else { "Duraklat" };
                if ui.button(label).clicked() {
                    if session.is_paused() {
                        handle.resume();
                    } else {
                        handle.pause();
                    }
                }
                if ui.button("Yeniden başlat").clicked() {
                    handle.restart();
                }
                if ui.button("Durdur").clicked() {
                    handle.cancel();
                }
            });
        });

        if ctx.input_mut(|i| i.consume_shortcut(&shortcut)) {
            if session.is_paused() {
                handle.resume();
            } else {
                handle.pause();
            }
        }

        // Keep the readouts live while the capture loop works on its own thread.
        if session.phase() == RecordPhase::Recording {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    });
}
