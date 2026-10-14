//! The launcher window: one input line, a live result, and history.

use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use eframe::egui::{self, FontId, Key, Modifiers, RichText};

use crate::calc::Calculator;
use crate::history::History;
use crate::rates;

/// Initial window size in logical pixels.
pub const WINDOW_SIZE: [f32; 2] = [640.0, 380.0];

/// egui's bundled fonts lack arrows and many math symbols (↑ √ ×), which then show as boxes.
/// DejaVu Sans covers them and ships with nearly every distro, at one of these paths
/// (Debian/Ubuntu, Fedora, Arch/openSUSE).
const FALLBACK_FONTS: [&str; 3] = [
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf",
    "/usr/share/fonts/TTF/DejaVuSans.ttf",
];

/// Adds the first of [`FALLBACK_FONTS`] found behind egui's own fonts; without one, text still
/// renders minus those glyphs.
fn install_fallback_font(ctx: &egui::Context) {
    let Some(bytes) = FALLBACK_FONTS
        .iter()
        .find_map(|path| std::fs::read(path).ok())
    else {
        tracing::warn!("no DejaVu Sans found; some symbols will show as boxes");
        return;
    };
    let families = [egui::FontFamily::Proportional, egui::FontFamily::Monospace]
        .into_iter()
        .map(|family| egui::epaint::text::InsertFontFamily {
            family,
            priority: egui::epaint::text::FontPriority::Lowest,
        })
        .collect();
    ctx.add_font(egui::epaint::text::FontInsert::new(
        "DejaVuSans",
        egui::FontData::from_owned(bytes),
        families,
    ));
}

/// Drops the empty IME preedit events IBus sends whenever egui reports the text cursor position.
///
/// egui-winit re-sends the cursor position on every frame that had any input event, and IBus
/// answers each time with an empty preedit, which is itself an input event: an endless ~75 fps
/// redraw loop while idle. An empty preedit only matters when it ends a composition in
/// progress (e.g. a cancelled Chinese input), so those are kept.
#[derive(Debug, Default)]
struct ImeFilter {
    composing: bool,
}

impl ImeFilter {
    /// Whether `event` should reach egui.
    fn keep(&mut self, event: &egui::Event) -> bool {
        match event {
            egui::Event::Ime(egui::ImeEvent::Preedit { text, .. }) if text.is_empty() => {
                std::mem::take(&mut self.composing)
            }
            egui::Event::Ime(egui::ImeEvent::Preedit { .. }) => {
                self.composing = true;
                true
            }
            egui::Event::Ime(egui::ImeEvent::Commit(_)) => {
                self.composing = false;
                true
            }
            _ => true,
        }
    }
}

/// Feedback below the input line.
enum Status {
    None,
    Copied(String),
    Error(String),
}

/// App state for one open launcher window.
pub struct SpotCalc {
    input: String,
    calc: Calculator,
    history: History,
    status: Status,
    /// Position while browsing history with ↑/↓; `None` when editing fresh input.
    browsing: Option<usize>,
    /// Set by the socket thread when the hotkey is pressed again.
    close_requested: Arc<AtomicBool>,
    /// Only close on focus loss after the window has actually had focus once.
    had_focus: bool,
    ime_filter: ImeFilter,
}

impl SpotCalc {
    /// Creates the app, starts listening for "close" toggles on `listener`, and loads
    /// exchange rates from `rates_cache` (refreshing them in the background if stale).
    #[must_use]
    pub fn new(
        ctx: &egui::Context,
        history: History,
        listener: UnixListener,
        rates_cache: PathBuf,
    ) -> Self {
        install_fallback_font(ctx);
        // A blinking cursor redraws the window several times a second; a steady one lets the
        // window sit idle with no redraws at all.
        ctx.all_styles_mut(|style| style.visuals.text_cursor.blink = false);
        // Redraw when fresh rates arrive so a currency preview fills in by itself.
        let repaint_ctx = ctx.clone();
        let rates = rates::start(rates_cache, move || repaint_ctx.request_repaint());
        let close_requested = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&close_requested);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            for conn in listener.incoming() {
                if conn.is_ok() {
                    flag.store(true, Ordering::Relaxed);
                    ctx.request_repaint();
                }
            }
        });
        Self {
            input: String::new(),
            calc: Calculator::with_rates(rates),
            history,
            status: Status::None,
            browsing: None,
            close_requested,
            had_focus: false,
            ime_filter: ImeFilter::default(),
        }
    }

    fn commit(&mut self, ctx: &egui::Context) {
        let input = self.input.trim().to_owned();
        if input.is_empty() {
            return;
        }
        match self.calc.evaluate(&input) {
            Ok(result) if result.is_empty() => {
                // Assignments like `x = 5` with no visible output.
                self.status = Status::None;
                self.input.clear();
            }
            Ok(result) => {
                ctx.copy_text(result.clone());
                if let Err(e) = self.history.push(&input, &result) {
                    tracing::warn!("could not save history: {e:#}");
                }
                self.status = Status::Copied(result);
                self.input.clear();
            }
            Err(e) => self.status = Status::Error(e),
        }
        self.browsing = None;
    }

    /// ↑ goes to older entries, ↓ to newer; past the newest clears the input.
    fn browse(&mut self, older: bool) {
        let len = self.history.entries().len();
        if len == 0 {
            return;
        }
        self.browsing = match (self.browsing, older) {
            (None, true) => Some(len - 1),
            (None, false) => None,
            (Some(i), true) => Some(i.saturating_sub(1)),
            (Some(i), false) if i + 1 < len => Some(i + 1),
            (Some(_), false) => None,
        };
        self.input = self
            .browsing
            .map(|i| self.history.entries()[i].input.clone())
            .unwrap_or_default();
    }
}

impl eframe::App for SpotCalc {
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        raw_input.events.retain(|event| self.ime_filter.keep(event));
    }

    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let focused = ctx.input(|i| i.viewport().focused);
        if focused == Some(true) {
            self.had_focus = true;
        }
        let lost_focus = self.had_focus && focused == Some(false);
        let escape = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
        if escape || lost_focus || self.close_requested.load(Ordering::Relaxed) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        tracing::trace!("frame");
        let ctx = ui.ctx().clone();
        // Consume these before the text field sees them, so ↑/↓ don't move the cursor
        // and Enter doesn't drop focus.
        let (enter, up, down) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::Enter),
                i.consume_key(Modifiers::NONE, Key::ArrowUp),
                i.consume_key(Modifiers::NONE, Key::ArrowDown),
            )
        });
        if enter {
            self.commit(&ctx);
        } else if up || down {
            self.browse(up);
        }

        let mut copy_from_history = None;
        // Undecorated windows have no border on GNOME, so draw one.
        let frame = egui::Frame::central_panel(ui.style())
            .inner_margin(16.0)
            .stroke(ui.visuals().widgets.noninteractive.bg_stroke);
        egui::CentralPanel::default().frame(frame).show(ui, |ui| {
            let edit = egui::TextEdit::singleline(&mut self.input)
                .font(FontId::proportional(28.0))
                .hint_text("Calculate…  e.g. 15% of 80, 3 ft to cm, 0xff to decimal")
                .desired_width(f32::INFINITY)
                .frame(egui::Frame::NONE);
            let response = ui.add(edit);
            // Only when needed: each request_focus also resets the input method, which makes
            // IBus send an event and so triggers another frame.
            if !response.has_focus() {
                response.request_focus();
            }
            if response.changed() {
                self.status = Status::None;
                self.browsing = None;
            }

            ui.add_space(4.0);
            let accent = ui.visuals().hyperlink_color;
            let weak = ui.visuals().weak_text_color();
            match (&self.status, self.calc.preview(&self.input)) {
                (Status::Error(e), _) => {
                    ui.label(RichText::new(e).color(ui.visuals().error_fg_color));
                }
                (_, Some(preview)) => {
                    ui.label(
                        RichText::new(format!("= {preview}"))
                            .size(22.0)
                            .color(accent),
                    );
                }
                (Status::Copied(r), None) => {
                    ui.label(RichText::new(format!("Copied {r}")).color(weak));
                }
                (Status::None, None) => {
                    ui.label(RichText::new(" ").size(22.0));
                }
            }

            ui.separator();
            egui::ScrollArea::vertical()
                .auto_shrink(false)
                .max_height(ui.available_height() - 22.0)
                .show(ui, |ui| {
                    for (i, entry) in self.history.entries().iter().enumerate().rev() {
                        let selected = self.browsing == Some(i);
                        let text = RichText::new(format!("{}  =  {}", entry.input, entry.result))
                            .monospace();
                        let row = ui.selectable_label(selected, text);
                        if row.clicked() {
                            copy_from_history = Some(entry.result.clone());
                        }
                        row.on_hover_text("Click to copy the result");
                    }
                });

            ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                ui.label(
                    RichText::new("Enter copy result · ↑↓ history · ans = last result · Esc close")
                        .small()
                        .color(weak),
                );
            });
        });

        if let Some(result) = copy_from_history {
            ctx.copy_text(result.clone());
            self.status = Status::Copied(result);
        }
    }

    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        visuals.panel_fill.to_normalized_gamma_f32()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn preedit(text: &str) -> egui::Event {
        egui::Event::Ime(egui::ImeEvent::Preedit {
            text: text.to_owned(),
            active_range_chars: None,
        })
    }

    #[test]
    fn idle_empty_preedits_are_dropped() {
        let mut filter = ImeFilter::default();
        assert!(!filter.keep(&preedit("")));
        assert!(!filter.keep(&preedit("")));
        assert!(filter.keep(&egui::Event::Text("1".to_owned())));
    }

    #[test]
    fn composition_events_pass_through() {
        let mut filter = ImeFilter::default();
        assert!(filter.keep(&preedit("ㄋㄧˇ")));
        assert!(filter.keep(&egui::Event::Ime(egui::ImeEvent::Commit("你".to_owned()))));
        assert!(
            !filter.keep(&preedit("")),
            "nothing left to clear after commit"
        );
    }

    #[test]
    fn cancelling_a_composition_clears_it() {
        let mut filter = ImeFilter::default();
        assert!(filter.keep(&preedit("ㄋ")));
        assert!(
            filter.keep(&preedit("")),
            "empty preedit ends the composition"
        );
        assert!(!filter.keep(&preedit("")));
    }
}
