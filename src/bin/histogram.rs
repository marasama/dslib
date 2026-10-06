use dslr::dslib::csv_parser::{read_csv, Column, CsvData};
use eframe::egui;
use egui_plot::{Bar, BarChart, Plot};
use std::{collections::HashMap, env, process::exit};

fn main() -> eframe::Result<()> {
    let arv: Vec<String> = env::args().collect();
    let filename = if arv.len() > 1 {
        &arv[1]
    } else {
        println!("Dosya bulunamadı!");
        exit(1);
    };
    //let data = csv_parser::read_csv::<f64>("dataset_test.csv", true).unwrap();
    //let a: f32 = arv[1].parse::<f32>().unwrap();
    //println!("{:?}", data.columns);

    let columns = read_csv(filename, true).unwrap();
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default(),
        ..Default::default()
    };

    eframe::run_native(
        "Zort",
        native_options,
        Box::new(|_cc| Ok(Box::new(LogisticRegression::new(columns)))),
    )
}

#[allow(unused)]
struct LogisticRegression {
    data: CsvData<f64>,
    categories: Vec<String>,
    selected_class_idx: u8,
    prev_sel_idx: u8,
}

fn classify(houses: &Column<f64>) -> Vec<String> {
    let mut categories: Vec<String> = Vec::new();

    match houses {
        Column::Text(v) => {
            for i in v.iter() {
                if !categories.contains(i) {
                    categories.push(i.clone());
                }
            }
        }
        Column::Number { .. } => {
            panic!("Wrong type of column to classify!");
        }
    }

    categories
}

impl LogisticRegression {
    pub fn new(data: CsvData<f64>) -> Self {
        let categories = classify(&data.columns[1]);
        println!("{:?}", categories);
        Self {
            data,
            categories,
            selected_class_idx: 6,
            prev_sel_idx: 0,
        }
    }
}

fn draw_histogram<'a>(houses: &Column<f64>, scores: &Column<f64>, categories: &[String]) {
    let (h, v, min, per_25, per_50, per_75, max): (Vec<String>, Vec<f64>, f64, f64, f64, f64, f64) =
        match (houses, scores) {
            (
                Column::Text(v),
                Column::Number {
                    rows,
                    min,
                    per_25,
                    per_50,
                    per_75,
                    max,
                    ..
                },
            ) => (
                v.clone(),
                rows.clone(),
                *min,
                *per_25,
                *per_50,
                *per_75,
                *max,
            ),

            (Column::Text(_), Column::Text(_)) => {
                panic!("Wrong data configuration!");
            }

            (Column::Number { .. }, Column::Text(_)) => {
                panic!("Wrong data configuration!");
            }

            (Column::Number { .. }, Column::Number { .. }) => {
                panic!("Wrong data configuration!");
            }
        };

    //let step_size = (max - min) / 100.0;
    let mut data: HashMap<&String, Vec<f64>> = HashMap::new();
    for i in categories.iter() {
        data.insert(i, Vec::new());
    }
    for (class, score) in h.iter().zip(v) {
        if let Some(v) = data.get_mut(class) {
            v.push(score);
        } else {
            panic!("There is a unclassified category in data!");
        }
    }

    for (i, k) in data.iter() {
        let mut chart = BarChart::new(*i, ((min as i32)..=(max as i32)).step_by(1).map());
    }
}

impl eframe::App for LogisticRegression {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::left("settings")
            .resizable(false)
            .default_size(400.0)
            .show(ui, |ctx| {
                ctx.label("Histogram");
                ctx.add_space(100.0);

                // Class seçimi
                let current_label = &self.data.headers[self.selected_class_idx as usize];
                egui::ComboBox::from_label("Class for histogram:")
                    .selected_text(current_label)
                    .show_ui(ctx, |ctx| {
                        for (idx, item) in self.data.headers.iter().enumerate().skip(6) {
                            ctx.selectable_value(&mut self.selected_class_idx, idx as u8, item);
                        }
                    });
            });
        if self.prev_sel_idx != self.selected_class_idx {
            draw_histogram(
                &self.data.columns[1],
                &self.data.columns[self.selected_class_idx as usize],
                &self.categories,
            );
            self.prev_sel_idx = self.selected_class_idx;
        }
        //egui::Panel::top("hist")
        //    .resizable(false)
        //    .default_size(400.0)
        //    .show(ui, |ctx| {
        //        ctx.label(hist);
        //    });
    }
}
