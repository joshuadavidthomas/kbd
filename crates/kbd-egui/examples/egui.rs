//! Opens an eframe/egui window and feeds keyboard events through a
//! `Matcher`. Press keys to see matches in the GUI.
//!
//! ```sh
//! cargo run -p kbd-egui --example egui
//! ```

use eframe::egui;
use kbd_core::Hotkey;
use kbd_core::Key;
use kbd_core::KeyTransition;
use kbd_core::MatchResult;
use kbd_core::Matcher;
use kbd_core::Modifier;
use kbd_egui::EguiEventExt;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([480.0, 360.0]),
        ..Default::default()
    };
    eframe::run_native(
        "kbd-egui example",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_pixels_per_point(1.5);
            Ok(Box::new(App::new()))
        }),
    )
}

struct App {
    matcher: Matcher,
    log: Vec<String>,
}

impl App {
    fn new() -> Self {
        let mut matcher = Matcher::new();

        matcher
            .register(Hotkey::new(Key::S).modifier(Modifier::Ctrl), || {})
            .expect("register Ctrl+S");
        matcher
            .register(Hotkey::new(Key::Z).modifier(Modifier::Ctrl), || {})
            .expect("register Ctrl+Z");
        matcher
            .register(
                Hotkey::new(Key::Z)
                    .modifier(Modifier::Ctrl)
                    .modifier(Modifier::Shift),
                || {},
            )
            .expect("register Ctrl+Shift+Z");
        matcher
            .register(Hotkey::new(Key::SPACE), || {})
            .expect("register Space");
        matcher
            .register(Hotkey::new(Key::ESCAPE), || {})
            .expect("register Escape");

        Self {
            matcher,
            log: Vec::new(),
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Process keyboard events
        for event in &ctx.input(|i| i.events.clone()) {
            let egui::Event::Key {
                pressed: true,
                repeat: false,
                ..
            } = event
            else {
                continue;
            };

            let Some(hotkey) = event.to_hotkey() else {
                continue;
            };

            // Standard app-level quit shortcut
            if hotkey == Hotkey::new(Key::Q).modifier(Modifier::Ctrl) {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                continue;
            }

            let line = match self.matcher.process(&hotkey, KeyTransition::Press) {
                MatchResult::Matched { .. } => format!("{hotkey} → matched!"),
                MatchResult::NoMatch => format!("{hotkey} → no match"),
                MatchResult::Pending { .. } => format!("{hotkey} → pending..."),
                MatchResult::Swallowed | MatchResult::Ignored => continue,
            };
            self.log.push(line);
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("kbd-egui example");
            ui.separator();

            ui.label("Registered bindings:");
            ui.monospace("  Ctrl+S        Save");
            ui.monospace("  Ctrl+Z        Undo");
            ui.monospace("  Ctrl+Shift+Z  Redo");
            ui.monospace("  Space         Space");
            ui.monospace("  Escape        Escape");
            ui.separator();

            ui.label("Press keys to see matches:");
            egui::ScrollArea::vertical()
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    for line in &self.log {
                        ui.monospace(line);
                    }
                });
        });
    }
}
