use dslr::dslib::csv_parser::{read_csv, Column, CsvData};
use eframe::egui;
use egui_plot::{Bar, BarChart, Legend, Plot, PlotPoints, Points};
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

enum GridCellData {
    Histogram(Vec<HistSeries>),
    ScatterPlot(Vec<Vec<[f64; 2]>>),
}

#[allow(unused)]
struct LogisticRegression {
    data: CsvData<f64>,
    categories: Vec<String>,
    selected_class_idx_1: u8,
    prev_sel_idx_1: u8,
    selected_class_idx_2: u8,
    prev_sel_idx_2: u8,
    charts: Vec<HistSeries>,
    points: Vec<Vec<[f64; 2]>>,
    pair_plots: Vec<GridCellData>,
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

        let mut pair_plots = Vec::with_capacity(169);

        for r in 0..13 {
            for c in 0..13 {
                let feat_col = c + 6;
                let feat_row = r + 6;
                let cell = if r == c {
                    GridCellData::Histogram(draw_histogram(
                        &data.columns[1],
                        &data.columns[feat_col],
                        &categories,
                    ))
                } else {
                    GridCellData::ScatterPlot(draw_scatter_plot(
                        &data,
                        feat_row,
                        feat_col,
                        &categories,
                    ))
                };
                pair_plots.push(cell);
            }
        }

        println!("{:?}", categories);
        Self {
            data,
            categories,
            selected_class_idx_1: 6,
            prev_sel_idx_1: 0,
            selected_class_idx_2: 7,
            prev_sel_idx_2: 0,
            charts: Vec::new(),
            points: Vec::new(),
            pair_plots,
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
    let (h, v, min, range): (Vec<String>, Vec<f64>, f64, f64) = match (houses, scores) {
        (
            Column::Text(v),
            Column::Number {
                rows, min, range, ..
            },
        ) => (v.clone(), rows.clone(), *min, *range),

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
    categories
        .iter()
        .enumerate()
        .map(|(idx_top, category)| {
            let vals = &data[category];
            let mut counts = vec![0.0; seg_count];
            for val in vals {
                let mut idx = ((val - min) / step_size).floor() as usize;
                if idx == seg_count {
                    idx = seg_count - 1
                };
                counts[idx] += 1.0;
            }
            let bar_data: Vec<(String, f64, f64)> = counts
                .iter()
                .enumerate()
                .map(|(idx, count)| {
                    let start = min + (idx as f64) * step_size;
                    let end = start + step_size;
                    let center = start + step_size / 2.0;
                    let label = format!("{:.1}-{:.1}", start, end);
                    (label, center, *count)
                })
                .collect();
            let bars: Vec<Bar> = bar_data
                .iter()
                .map(|(s, x, c)| Bar::new(*x, *c).width(step_size).name(s.as_str()))
                .collect();
            let color = class_color(idx_top, 255);
            (category.to_string(), color, bars)
        })
        .collect()
}

fn draw_scatter_plot(
    data: &CsvData<f64>,
    idx_1: usize,
    idx_2: usize,
    categories: &[String],
) -> Vec<Vec<[f64; 2]>> {
    let classes = match &data.columns[1] {
        Column::Text(s) => s,
        Column::Number { .. } => panic!("Wrong data configuration!"),
    };

    let (col_1, col_2): (Vec<f64>, Vec<f64>) = match (&data.columns[idx_1], &data.columns[idx_2]) {
        (Column::Number { rows: rows_1, .. }, Column::Number { rows: rows_2, .. }) => {
            (rows_1.clone(), rows_2.clone())
        }

        (Column::Text(_), Column::Text(_)) => {
            panic!("Wrong data configuration!");
        }

        (Column::Number { .. }, Column::Text(_)) => {
            panic!("Wrong data configuration!");
        }

        (Column::Text { .. }, Column::Number { .. }) => {
            panic!("Wrong data configuration!");
        }
    };

    categories
        .iter()
        .map(|c| {
            col_1
                .iter()
                .zip(col_2.iter().zip(classes.iter()))
                .filter_map(|(a, (b, s))| if *s == *c { Some([*a, *b]) } else { None })
                .collect()
        })
        .collect()
}

impl eframe::App for LogisticRegression {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("hist").resizable(true).show(ui, |ctx| {
            ctx.label("Zort");
        });
        egui::CentralPanel::default().show(ui, |ctx| {
            let available = ctx.available_size();
            let label_width = 110.0;
            let header_height = 24.0;
            let plot_w = (available.x - label_width).max(0.0);
            let plot_h = (available.y - header_height).max(0.0);
            let cell_size = (plot_w / 13.0).min(plot_h / 13.0);

            // Sütun Başlıkları (Üst Satır)
            ctx.horizontal(|h_ui| {
                h_ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                h_ui.allocate_space(egui::vec2(label_width, header_height));
                for col in 0..13 {
                    h_ui.allocate_ui_with_layout(
                        egui::vec2(cell_size, header_height),
                        egui::Layout::centered_and_justified(egui::Direction::TopDown),
                        |ui| {
                            ui.add(egui::Label::new(
                                egui::RichText::new(&self.data.headers[col + 6])
                                    .size(8.5)
                                    .strong(),
                            ))
                            .on_hover_text(&self.data.headers[col + 6]);
                        },
                    );
                }
            });

            // Satırlar ve Grafikler
            for rows in 0..13 {
                ctx.horizontal(|row_ui| {
                    row_ui.spacing_mut().item_spacing = egui::Vec2::ZERO;

                    // Satır Başlığı (Sol Sütun)
                    row_ui.allocate_ui_with_layout(
                        egui::vec2(label_width, cell_size),
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            ui.add_space(4.0);
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(&self.data.headers[rows + 6])
                                        .size(9.0)
                                        .strong(),
                                )
                                .truncate(),
                            )
                            .on_hover_text(&self.data.headers[rows + 6]);
                        },
                    );

                    for cols in 0..13 {
                        Plot::new(format!("{}_{}", rows, cols))
                            .width(cell_size)
                            .height(cell_size)
                            .show_y(false)
                            .show_x(false)
                            .show_axes(false)
                            .show_grid(false)
                            .allow_zoom(false)
                            .allow_drag(false)
                            .allow_scroll(false)
                            .show(row_ui, |plot_ui| match self.pair_plots[rows * 13 + cols] {
                                GridCellData::Histogram(ref h_data) => {
                                    for (a, s, d) in h_data.iter() {
                                        plot_ui.bar_chart(BarChart::new(a, d.clone()).color(*s));
                                    }
                                }
                                GridCellData::ScatterPlot(ref s_data) => {
                                    for (idx, v) in s_data.iter().enumerate() {
                                        let points: PlotPoints = v.clone().into();
                                        plot_ui.points(
                                            Points::new(self.categories[idx].clone(), points)
                                                .radius(1.0)
                                                .color(class_color(idx, 255)),
                                        );
                                    }
                                }
                            });
                    }
                });
            }
        });
    }
}
