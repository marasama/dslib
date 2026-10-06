#[cfg(test)]
mod csv_tests {
    use crate::dslib::csv_parser::{self, Column};

    #[test]
    fn test_parse_dataset_test() {
        let data = csv_parser::read_csv::<f64>("dataset_test.csv", true).expect("Failed to read test dataset");
        assert_eq!(data.row_count, 400);
        assert_eq!(data.headers.len(), 19);
        assert_eq!(data.columns.len(), 19);

        // Hogwarts House is empty in test dataset, should be inferred as Text
        match &data.columns[1] {
            Column::Text(v) => {
                assert_eq!(v.len(), 400);
                assert!(v.iter().all(|s| s.is_empty()));
            }
            Column::Number(_) => panic!("Hogwarts House in test dataset should be Text, not Number!"),
        }

        // Arithmancy is numerical
        match &data.columns[6] {
            Column::Number(v) => assert_eq!(v.len(), 400),
            Column::Text(_) => panic!("Arithmancy should be Number!"),
        }
    }

    #[test]
    fn test_parse_dataset_train() {
        let data = csv_parser::read_csv::<f64>("dataset_train.csv", true).expect("Failed to read train dataset");
        assert_eq!(data.row_count, 1600);
        assert_eq!(data.headers.len(), 19);
        assert_eq!(data.columns.len(), 19);

        // Hogwarts House is populated in train dataset
        match &data.columns[1] {
            Column::Text(v) => {
                assert_eq!(v.len(), 1600);
                assert_eq!(v[0], "Ravenclaw");
                assert_eq!(v[1], "Slytherin");
            }
            Column::Number(_) => panic!("Hogwarts House in train dataset should be Text!"),
        }
    }

    #[test]
    fn test_quotes_and_escapes() {
        let csv_data = b"name,score,note\n\"Doe, John\",95.5,\"He said \"\"Hello\"\"\"\n\"Smith, Jane\",88.0,\"Normal note\"\n";
        let parsed = csv_parser::parse_csv::<f64>(csv_data, true).unwrap();
        assert_eq!(parsed.row_count, 2);
        assert_eq!(parsed.headers, vec!["name", "score", "note"]);

        match &parsed.columns[0] {
            Column::Text(v) => assert_eq!(v, &vec!["Doe, John", "Smith, Jane"]),
            _ => panic!("Expected text"),
        }
        match &parsed.columns[1] {
            Column::Number(v) => assert_eq!(v, &vec![95.5, 88.0]),
            _ => panic!("Expected number"),
        }
        match &parsed.columns[2] {
            Column::Text(v) => assert_eq!(v, &vec!["He said \"Hello\"", "Normal note"]),
            _ => panic!("Expected text"),
        }
    }
}
