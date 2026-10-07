use dslr::dslib::csv_parser::{read_csv, Column, CsvData};
use eframe::egui;
use egui_plot::{Bar, BarChart, Legend, Plot};
use std::{collections::HashMap, env, fmt::format, process::exit};

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
    charts: Vec<HistSeries>,
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
            charts: Vec::new(),
        }
    }
}

pub type HistSeries = (String, egui::Color32, Vec<Bar>);

fn class_color(ci: usize, alpha: u8) -> egui::Color32 {
    let hue = (ci as f32 * 0.618_034) % 1.0; // altın oran: komşu renkler birbirinden uzak kalır
    let [r, g, b] = eframe::epaint::Hsva::new(hue, 0.6, 0.8, 1.0).to_srgb();
    egui::Color32::from_rgba_unmultiplied(r, g, b, alpha)
}

fn draw_histogram(
    houses: &Column<f64>,
    scores: &Column<f64>,
    categories: &[String],
) -> Vec<HistSeries> {
    let (h, v, min, max, range): (Vec<String>, Vec<f64>, f64, f64, f64) = match (houses, scores) {
        (
            Column::Text(v),
            Column::Number {
                rows,
                min,
                max,
                range,
                ..
            },
        ) => (v.clone(), rows.clone(), *min, *max, *range),

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

    let seg_count = 10;
    let step_size = range / seg_count as f64;
    //let bars: Vec<BarChart> = data
    data.iter()
        .enumerate()
        .map(|(idx_top, (category, vals))| {
            let mut counts = vec![0.0; seg_count];
            for &val in vals {
                let mut idx = ((val - min) / step_size).floor() as usize;
                if idx == seg_count {
                    idx = seg_count - 1
                };
                counts[idx] += 1.0;
            }
            let bar_data: Vec<(String, f64)> = counts
                .iter()
                .enumerate()
                .map(|(idx, count)| {
                    let start = min + (idx as f64) * step_size;
                    let end = start + step_size;
                    let label = format!("{:.1}-{:.1}", start, end);
                    (label, *count)
                })
                .collect();
            let bars: Vec<Bar> = bar_data
                .iter()
                .enumerate()
                .map(|(idx, (s, c))| Bar::new(idx as f64, *c).name(s.as_str()))
                .collect();
        })
        .collect()
    //for (i, k) in data.iter() {
    //    let mut counts = vec![0.0; seg_count];
    //    for &val in k {
    //        let mut idx = ((val - min) / step_size).floor() as usize;
    //        if idx == seg_count {
    //            idx = seg_count - 1
    //        };
    //        counts[idx] += 1.0;
    //    }
    //    let bar_data: Vec<(String, f64)> = counts
    //        .iter()
    //        .enumerate()
    //        .map(|(idx, count)| {
    //            let start = min + (idx as f64) / step_size;
    //            let end = start + step_size;
    //            let label = format!("{:.1}-{:.1}", start, end);
    //            (label, *count)
    //        })
    //        .collect();
    //    let data: Vec<(&str, f64)> = bar_data.iter().map(|(s, c)| (s.as_str(), *c)).collect();
    //    let chart = BarChart::new(
    //        *i,
    //        data.iter()
    //            .enumerate()
    //            .map(|(idx, (s, v))| Bar::new(idx as f64, *v).name(s))
    //            .collect(),
    //    );
    //}
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
        egui::CentralPanel::default().show(ui, |ctx| {
            if self.prev_sel_idx != self.selected_class_idx {
                self.charts = draw_histogram(
                    &self.data.columns[1],
                    &self.data.columns[self.selected_class_idx as usize],
                    &self.categories,
                );
                self.prev_sel_idx = self.selected_class_idx;
            }
            Plot::new("histogram")
                .legend(Legend::default())
                .show(ctx, |plot_ui| {
                    for (a, s, d) in self.charts.iter() {
                        plot_ui.bar_chart(BarChart::new(a, d.clone()).color(*s));
                    }
                });
        });
        //egui::Panel::top("hist")
        //    .resizable(false)
        //    .default_size(400.0)
        //    .show(ui, |ctx| {
        //        ctx.label(hist);
        //    });
    }
}
