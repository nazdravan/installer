//! Fereastra instalatorului: lista de programe cu bife și instalarea lor pe un fir separat.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use egui::{Color32, Margin, RichText, Ui};

use crate::catalog::Entry;
use crate::command::{self, Action};
use crate::detect::{self, Registry};
use crate::exec::{self, Outcome};
use crate::journal::Journal;
use crate::manifest::Variant;
use crate::select::{self, Status};
use crate::system::SystemInfo;

#[derive(Clone, Debug, PartialEq)]
pub enum Progress {
    Pending,
    Running,
    Done(Outcome),
}

pub struct Row {
    pub entry: Entry,
    pub status: Status,
    pub checked: bool,
    /// Indicele variantei potrivite în `program.variants`.
    pub variant: Option<usize>,
    pub progress: Progress,
}

pub enum Msg {
    Started(usize),
    Finished(usize, Outcome),
    AllDone,
}

pub fn build_rows(entries: Vec<Entry>, sys: &SystemInfo, reg: &dyn Registry) -> Vec<Row> {
    entries
        .into_iter()
        .map(|entry| {
            let (variant, installed, default) = match &entry.program {
                Ok(p) => {
                    let v = select::choose_variant(p, sys);
                    let index = v.and_then(|v| p.variants.iter().position(|x| std::ptr::eq(x, v)));
                    (index, v.and_then(|v| detect::installed_version(p, v, reg)), p.default)
                }
                Err(_) => (None, None, false),
            };
            let status = select::status(&entry.program, sys, installed.as_deref());
            let checked = status.checkable() && status.default_checked(default);
            Row { entry, status, checked, variant, progress: Progress::Pending }
        })
        .collect()
}

/// De exemplu „5 instalate, 1 eroare, repornire necesară”.
pub fn summary(rows: &[Row]) -> String {
    let outcomes = || rows.iter().filter_map(|r| match &r.progress {
        Progress::Done(o) => Some(o),
        _ => None,
    });
    let ok = outcomes().filter(|o| !matches!(o, Outcome::Failed(_))).count();
    let failed = outcomes().filter(|o| matches!(o, Outcome::Failed(_))).count();
    let mut text = if ok == 1 { "1 instalat".to_string() } else { format!("{ok} instalate") };
    match failed {
        0 => {}
        1 => text += ", 1 eroare",
        n => text += &format!(", {n} erori"),
    }
    if outcomes().any(|o| *o == Outcome::OkReboot) {
        text += ", repornire necesară";
    }
    text
}

enum State {
    Choosing,
    Running(Receiver<Msg>),
    Finished,
}

pub struct InstallerApp {
    sys: SystemInfo,
    journal_dir: PathBuf,
    rows: Vec<Row>,
    state: State,
}

impl InstallerApp {
    pub fn new(sys: SystemInfo, journal_dir: PathBuf, rows: Vec<Row>) -> Self {
        InstallerApp { sys, journal_dir, rows, state: State::Choosing }
    }

    fn checked_count(&self) -> usize {
        self.rows.iter().filter(|r| r.checked).count()
    }

    fn set_all(&mut self, checked: bool) {
        for row in self.rows.iter_mut().filter(|r| r.status.checkable()) {
            row.checked = checked;
        }
    }

    fn start(&mut self) {
        let mut jobs: Vec<(usize, Action, Variant, String)> = Vec::new();
        for (i, row) in self.rows.iter_mut().enumerate() {
            row.progress = Progress::Pending;
            let (true, Ok(p), Some(v)) = (row.checked, &row.entry.program, row.variant) else { continue };
            let v = &p.variants[v];
            jobs.push((i, command::action_for(p, v, &row.entry.folder), v.clone(), p.name.clone()));
        }
        let mut journal = Journal::open(&self.journal_dir, &self.sys.computer);
        journal.line(&self.sys.label());
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            for (i, action, variant, name) in jobs {
                let _ = tx.send(Msg::Started(i));
                journal.line(&format!("{name}: {}", action.describe()));
                let outcome = exec::execute(&action, &variant);
                journal.line(&format!("{name}: {outcome:?}"));
                let _ = tx.send(Msg::Finished(i, outcome));
            }
            let _ = tx.send(Msg::AllDone);
        });
        self.state = State::Running(rx);
    }

    fn poll(&mut self) {
        let State::Running(rx) = &self.state else { return };
        let mut done = false;
        while let Ok(msg) = rx.try_recv() {
            match msg {
                Msg::Started(i) => self.rows[i].progress = Progress::Running,
                Msg::Finished(i, o) => self.rows[i].progress = Progress::Done(o),
                Msg::AllDone => done = true,
            }
        }
        if done {
            self.state = State::Finished;
        }
    }

    fn row_ui(ui: &mut Ui, row: &mut Row, enabled: bool) {
        let Ok(p) = &row.entry.program else {
            let folder = row.entry.folder.file_name().unwrap_or_default().to_string_lossy();
            ui.horizontal(|ui| {
                ui.add_enabled(false, egui::Checkbox::new(&mut false, RichText::new(folder).strong()));
                ui.label(RichText::new(row.status.label("")).color(Color32::from_rgb(200, 60, 60)));
            });
            return;
        };
        ui.horizontal(|ui| {
            let title = RichText::new(format!("{} {}", p.name, p.version)).strong();
            ui.add_enabled(enabled && row.status.checkable(), egui::Checkbox::new(&mut row.checked, title));
            ui.label(RichText::new(row.status.label(&p.version)).weak());
            match &row.progress {
                Progress::Pending => {}
                Progress::Running => {
                    ui.spinner();
                }
                Progress::Done(Outcome::Ok) => {
                    ui.label(RichText::new("✔").color(Color32::from_rgb(40, 160, 70)).strong());
                }
                Progress::Done(Outcome::OkReboot) => {
                    ui.label(RichText::new("✔ (repornire)").color(Color32::from_rgb(40, 160, 70)).strong());
                }
                Progress::Done(Outcome::Failed(why)) => {
                    ui.label(RichText::new(format!("❌ {why}")).color(Color32::from_rgb(200, 60, 60)).strong());
                }
            }
        });
        if !p.description.is_empty() {
            ui.indent(&p.name, |ui| ui.label(RichText::new(&p.description).small().weak()));
        }
    }
}

impl eframe::App for InstallerApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll();
        if matches!(self.state, State::Running(_)) {
            ctx.request_repaint_after(Duration::from_millis(200));
        }
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let choosing = matches!(self.state, State::Choosing | State::Finished);

        egui::Panel::top("sistem").frame(egui::Frame::new().fill(ui.visuals().panel_fill).inner_margin(Margin::symmetric(14, 10))).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Instalator școală").size(20.0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(self.sys.label()).weak());
                });
            });
        });

        egui::Panel::bottom("butoane").frame(egui::Frame::new().fill(ui.visuals().panel_fill).inner_margin(Margin::symmetric(14, 10))).show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.add_enabled(choosing, egui::Button::new("Toate")).clicked() {
                    self.set_all(true);
                }
                if ui.add_enabled(choosing, egui::Button::new("Nimic")).clicked() {
                    self.set_all(false);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let n = self.checked_count();
                    let install = egui::Button::new(RichText::new(format!("Instalează ({n})")).strong());
                    if ui.add_enabled(choosing && n > 0, install).clicked() {
                        self.start();
                    }
                    match self.state {
                        State::Running(_) => {
                            ui.spinner();
                        }
                        State::Finished => {
                            ui.label(summary(&self.rows));
                        }
                        State::Choosing => {}
                    }
                });
            });
        });

        egui::CentralPanel::default().frame(egui::Frame::new().fill(ui.visuals().extreme_bg_color).inner_margin(Margin::same(14))).show(ui, |ui| {
            if self.rows.is_empty() {
                ui.label("Nu am găsit programe în mapa „Windows” de lângă instalator.");
                return;
            }
            egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                let mut category: Option<String> = None;
                for row in &mut self.rows {
                    let this = row.entry.program.as_ref().map(|p| p.category.clone()).unwrap_or_else(|_| "Cu erori".into());
                    if category.as_ref() != Some(&this) {
                        ui.add_space(if category.is_some() { 10.0 } else { 0.0 });
                        ui.label(RichText::new(this.to_uppercase()).small().weak());
                        category = Some(this);
                    }
                    Self::row_ui(ui, row, choosing);
                }
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system::{Arch, WinVer};

    struct NoRegistry;
    impl Registry for NoRegistry {
        fn uninstall_entries(&self) -> Vec<(String, String)> {
            Vec::new()
        }
    }

    fn sys() -> SystemInfo {
        SystemInfo { win: Some(WinVer::Win10), arch: Arch::X64, admin: true, computer: "PC".into(), linux: None }
    }

    fn entry(default: bool) -> Entry {
        let program = toml::from_str(&format!(
            "name = \"P\"\nversion = \"1\"\ncategory = \"C\"\ndefault = {default}\n[[variant]]\nwindows = [\"10\"]\narch = \"x86\"\nkind = \"zip\"\nfile = \"f.zip\"\ntarget = '/nu/exista/P'\nshortcut = \"p.exe\"\n"
        ))
        .unwrap();
        Entry { folder: "P".into(), program: Ok(program) }
    }

    fn done(outcomes: &[Outcome]) -> Vec<Row> {
        let mut rows = build_rows(outcomes.iter().map(|_| entry(true)).collect(), &sys(), &NoRegistry);
        for (row, o) in rows.iter_mut().zip(outcomes) {
            row.progress = Progress::Done(o.clone());
        }
        rows
    }

    #[test]
    fn summary_text() {
        let fail = || Outcome::Failed("x".into());
        let mut many = vec![Outcome::Ok; 4];
        many.extend([Outcome::OkReboot, fail()]);
        assert_eq!(summary(&done(&many)), "5 instalate, 1 eroare, repornire necesară");
        assert_eq!(summary(&done(&[Outcome::Ok, Outcome::Ok])), "2 instalate");
        assert_eq!(summary(&done(&[Outcome::Ok, fail(), fail()])), "1 instalat, 2 erori");
    }

    #[test]
    fn build_rows_defaults() {
        let invalid = Entry { folder: "Z".into(), program: Err("lipsește program.toml".into()) };
        let rows = build_rows(vec![entry(true), entry(false), invalid], &sys(), &NoRegistry);
        assert!(rows[0].checked);
        assert_eq!(rows[0].variant, Some(0));
        assert_eq!(rows[0].status, Status::Missing);
        assert!(!rows[1].checked);
        assert!(!rows[2].checked);
        assert_eq!(rows[2].variant, None);
        assert_eq!(rows[2].progress, Progress::Pending);
    }
}
