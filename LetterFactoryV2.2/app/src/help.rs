//! Help window showing the bundled user guide. Port of the Java `HelpWindow`.

use crate::theme;

const GUIDE: &str = include_str!("../assets/user_guide.txt");

#[derive(Default)]
pub struct HelpWindow {
    pub open: bool,
}

impl HelpWindow {
    pub fn ui(&mut self, ctx: &egui::Context) {
        if !self.open {
            return;
        }
        let mut open = self.open;
        egui::Window::new("LetterFactory — Help")
            .open(&mut open)
            .resizable(true)
            .default_width(560.0)
            .default_height(560.0)
            .frame(theme::window_frame())
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(theme::title_text("LetterFactory"));
                    ui.label(theme::muted_text("User Guide"));
                });
                ui.add_space(6.0);
                ui.separator();
                ui.add_space(6.0);
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.label(theme::body_text(GUIDE));
                    });
                ui.add_space(6.0);
                ui.separator();
                ui.vertical_centered(|ui| {
                    ui.label(theme::muted_text("Developed by Ethan Silvio  ·  Version 2.2"));
                });
            });
        self.open = open;
    }
}
