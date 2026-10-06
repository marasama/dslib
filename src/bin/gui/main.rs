use eframe::egui;
use egui_plot;
use std::env;

fn main() -> eframe::Result<()> {
    //let arv: Vec<String> = env::args().collect();
    //let data = csv_parser::read_csv::<f64>("dataset_test.csv", true).unwrap();
    //let a: f32 = arv[1].parse::<f32>().unwrap();
    //println!("{:?}", data.columns);
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default(),
        ..Default::default()
    };

    eframe::run_native(
        "Zort",
        native_options,
        Box::new(|_cc| Ok(Box::new(LogisticRegression::default()))),
    )
}

struct LogisticRegression {
    describe: bool,
}

impl Default for LogisticRegression {
    fn default() -> Self {
        Self { describe: false }
    }
}

impl eframe::App for LogisticRegression {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        egui::Panel::left("controls")
            .resizable(false)
            .default_size(220.0)
            .show(ui, |ctx| {
                ctx.heading("Zort");
                ctx.add_space(100.0);
                ctx.heading("Zart");
            });
    }
}
