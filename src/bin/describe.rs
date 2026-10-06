use dslr::dslib::csv_parser::{read_csv, Column};
use std::env;
use std::process::exit;

fn main() {
    // dataset_train.csv - Pandas ile karşılaştırma için istatistik doğrulaması
    let arv: Vec<String> = env::args().collect();
    let filename = if arv.len() > 1 {
        &arv[1]
    } else {
        println!("Dosya bulunamadı!");
        exit(1);
    };
    let data = read_csv::<f64>(filename, true).expect("CSV okunamadı");

    // Sütun indeksleri: Index=0, HogwartsHouse=1, ..., Arithmancy=6
    let col_names = &data.headers;

    println!(
        "{:<30} {:>8} {:>14} {:>12} {:>12} {:>12} {:>12} {:>12} {:>12} {:>12}",
        "Sütun", "Count", "Mean", "Std", "Min", "25%", "50%", "75%", "Max", "Range"
    );
    println!("{}", "-".repeat(150));

    for (i, col) in data.columns.iter().enumerate() {
        if let Column::Number {
            count,
            mean,
            std,
            min,
            per_25,
            per_50,
            per_75,
            max,
            range,
            ..
        } = col
        {
            println!(
                "{:<30} {:>8} {:>14.4} {:>12.4} {:>12.4} {:>12.4} {:>12.4} {:>12.4} {:>12.4} {:>12.4}",
                col_names[i], count, mean, std, min, per_25, per_50, per_75, max, range
            );
        }
    }
}
