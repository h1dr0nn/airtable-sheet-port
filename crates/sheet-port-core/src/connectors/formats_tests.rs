use super::*;
use crate::connectors::{parse_cells_range, ConnectorRegistry};
use crate::constants::READ_FORMATS_MAX_CELLS;
use crate::test_fixtures::{demo_db, DEMO_SOURCE_ID, DEMO_TABLE_ID};

fn background_only() -> FormatFields {
    FormatFields {
        background: true,
        ..FormatFields::default()
    }
}

fn filled(color: &str) -> CellSample {
    CellSample {
        background: Some(color.to_string()),
        ..CellSample::default()
    }
}

fn request(range: Option<&str>, fields: FormatFields, max_cells: usize) -> FormatsRequest {
    FormatsRequest {
        range: range.map(|range| parse_cells_range(range).unwrap()),
        fields,
        source: FormatSource::Effective,
        max_cells,
    }
}

#[test]
fn runs_encode_counts_and_trim_the_default_tail() {
    assert_eq!(encode_runs(&[]), "");
    assert_eq!(encode_runs(&[0, 0, 0]), "");
    assert_eq!(encode_runs(&[1]), "1");
    assert_eq!(encode_runs(&[0, 0, 3, 3, 3, 1, 0, 0]), "0*2,3*3,1");
    assert_eq!(encode_runs(&[2, 0, 2]), "2,0,2");
    assert_eq!(encode_runs(&[10, 10, 11]), "10*2,11");
}

#[test]
fn runs_round_trip_through_decode() {
    let rows: [&[usize]; 6] = [
        &[],
        &[0, 0, 0, 0],
        &[1, 1, 1, 1],
        &[0, 3, 3, 0, 12, 12, 12, 0, 0],
        &[5],
        &[0, 1, 0, 1, 0, 1, 0],
    ];
    for row in rows {
        let encoded = encode_runs(row);
        assert_eq!(decode_runs(&encoded, row.len()).unwrap(), row, "{encoded}");
    }
    assert_eq!(decode_runs("", 3).unwrap(), vec![0, 0, 0]);
    assert!(decode_runs("1*4", 3).is_err(), "longer than the row");
    for bad in ["x", "1*", "*2", "1*0", "1,,2", "1*2*3"] {
        assert!(decode_runs(bad, 10).is_err(), "{bad}");
    }
}

#[test]
fn layers_put_the_default_first_and_count_every_window_cell() {
    let grid = FormatGrid {
        sheet_title: "Levels".to_string(),
        start_row: 0,
        start_col: 0,
        rows: 3,
        columns: 4,
        // Ragged: row 1 is short, row 2 is missing entirely.
        cells: vec![
            vec![
                filled("#ff0000"),
                filled("#ff0000"),
                CellSample::default(),
                filled("#00ff00"),
            ],
            vec![filled("#00ff00")],
        ],
    };
    let output = encode_formats(&grid, &background_only(), FormatSource::Effective);
    let layer = output.background.expect("background layer");
    assert_eq!(
        layer.palette,
        vec![
            None,
            Some("#ff0000".to_string()),
            Some("#00ff00".to_string())
        ]
    );
    assert_eq!(layer.counts, vec![8, 2, 2]);
    assert_eq!(layer.grid, vec!["1*2,0,2", "2", ""]);
    assert_eq!(output.range, "A1:D3");
    assert_eq!(output.start_column, "A");
    assert!(output.bold.is_none() && output.value.is_none());
    for row in &layer.grid {
        assert_eq!(decode_runs(row, grid.columns).unwrap().len(), 4);
    }
}

#[test]
fn values_are_trimmed_rows_and_booleans_use_false_as_default() {
    let fields = FormatFields {
        bold: true,
        value: true,
        ..FormatFields::default()
    };
    let bold_label = CellSample {
        bold: true,
        value: "F".to_string(),
        ..CellSample::default()
    };
    let grid = FormatGrid {
        sheet_title: "T".to_string(),
        start_row: 9,
        start_col: 1,
        rows: 2,
        columns: 3,
        cells: vec![vec![
            CellSample::default(),
            bold_label,
            CellSample::default(),
        ]],
    };
    let output = encode_formats(&grid, &fields, FormatSource::UserEntered);
    assert_eq!(output.range, "B10:D11");
    assert_eq!(output.start_row, 10);
    assert_eq!(output.source, "userEntered");
    let bold = output.bold.expect("bold layer");
    assert_eq!(bold.palette, vec![false, true]);
    assert_eq!(bold.counts, vec![5, 1]);
    assert_eq!(bold.grid, vec!["0,1", ""]);
    assert_eq!(
        output.value.expect("value layer").grid,
        vec![vec!["".to_string(), "F".to_string()], vec![]]
    );
}

#[test]
fn an_empty_window_reports_its_start_cell() {
    let grid = FormatGrid {
        sheet_title: "T".to_string(),
        start_row: 0,
        start_col: 0,
        rows: 0,
        columns: 0,
        cells: Vec::new(),
    };
    let output = encode_formats(&grid, &background_only(), FormatSource::Effective);
    assert_eq!(output.range, "A1");
    let layer = output.background.unwrap();
    assert_eq!(layer.palette, vec![None]);
    assert_eq!(layer.counts, vec![0]);
    assert!(layer.grid.is_empty());
}

#[test]
fn open_windows_trim_to_the_used_cells_and_bounded_ones_keep_their_size() {
    let cells = vec![
        vec![CellSample::default(), filled("#111111")],
        vec![CellSample::default()],
        vec![
            filled("#222222"),
            CellSample::default(),
            CellSample::default(),
        ],
        vec![CellSample::default()],
    ];
    let whole = shape_format_grid(
        "T".to_string(),
        &request(None, background_only(), 100),
        Some(1000),
        Some(26),
        cells.clone(),
    )
    .unwrap();
    assert_eq!((whole.rows, whole.columns), (3, 2));
    assert_eq!(whole.cells.len(), 3);
    assert!(whole.cells.iter().all(|row| row.len() <= 2));

    let bounded = shape_format_grid(
        "T".to_string(),
        &request(Some("A1:E10"), background_only(), 100),
        Some(1000),
        Some(26),
        cells.clone(),
    )
    .unwrap();
    assert_eq!((bounded.rows, bounded.columns), (10, 5));

    // A bounded range past the grid is clamped to it.
    let clamped = shape_format_grid(
        "T".to_string(),
        &request(Some("B2:Z99"), background_only(), 10_000),
        Some(20),
        Some(10),
        Vec::new(),
    )
    .unwrap();
    assert_eq!((clamped.rows, clamped.columns), (19, 9));
    assert_eq!((clamped.start_row, clamped.start_col), (1, 1));

    // Whole columns: rows trimmed, columns kept.
    let columns = shape_format_grid(
        "T".to_string(),
        &request(Some("A:D"), background_only(), 100),
        None,
        None,
        cells,
    )
    .unwrap();
    assert_eq!((columns.rows, columns.columns), (3, 4));
}

#[test]
fn value_only_requests_ignore_formats_when_trimming() {
    let fields = FormatFields {
        value: true,
        ..FormatFields::default()
    };
    let cells = vec![vec![
        CellSample {
            value: "x".to_string(),
            ..CellSample::default()
        },
        filled("#ff0000").masked(&fields),
    ]];
    let grid = shape_format_grid(
        "T".to_string(),
        &request(None, fields, 100),
        None,
        None,
        cells,
    )
    .unwrap();
    assert_eq!((grid.rows, grid.columns), (1, 1));
}

#[test]
fn the_cell_cap_rejects_before_and_after_the_fetch() {
    let error = check_fetch_size(parse_cells_range("A1:J10").ok().as_ref(), 1000, 26, 50)
        .expect_err("100 cells over a cap of 50");
    let message = error.to_string();
    assert!(message.contains("100 cells"), "{message}");
    assert!(message.contains("saveTo"), "{message}");
    // Clamped to the grid first.
    assert!(check_fetch_size(parse_cells_range("A1:J10").ok().as_ref(), 5, 10, 50).is_ok());
    // Open windows only need to stay under the saveTo cap before the fetch.
    assert!(check_fetch_size(None, 1000, 391, READ_FORMATS_MAX_CELLS).is_ok());
    assert!(check_fetch_size(None, 100_000, 702, READ_FORMATS_MAX_CELLS).is_err());

    let cells = vec![vec![filled("#ff0000"); 10]; 10];
    let error = shape_format_grid(
        "T".to_string(),
        &request(None, background_only(), 50),
        None,
        None,
        cells,
    )
    .expect_err("trimmed grid still over the cap");
    assert!(matches!(error, CoreError::InvalidInput(_)));
    let saved = too_many_cells(3_000_000, READ_FORMATS_MAX_CELLS_SAVED).to_string();
    assert!(!saved.contains("saveTo"), "{saved}");
}

#[test]
fn masked_samples_keep_only_requested_properties() {
    let full = CellSample {
        background: Some("#ff0000".to_string()),
        font_color: Some("#000000".to_string()),
        bold: true,
        italic: true,
        strikethrough: true,
        underline: true,
        font_family: Some("Lexend".to_string()),
        font_size: Some(12),
        vertical_alignment: Some("MIDDLE".to_string()),
        value: "v".to_string(),
    };
    let masked = full.clone().masked(&FormatFields {
        italic: true,
        ..FormatFields::default()
    });
    assert_eq!(
        masked,
        CellSample {
            italic: true,
            ..CellSample::default()
        }
    );
    let font = full.clone().masked(&FormatFields {
        font_family: true,
        vertical_alignment: true,
        ..FormatFields::default()
    });
    assert_eq!(
        font,
        CellSample {
            font_family: Some("Lexend".to_string()),
            vertical_alignment: Some("MIDDLE".to_string()),
            ..CellSample::default()
        }
    );
    assert_eq!(
        FormatFields {
            underline: true,
            font_family: true,
            font_size: true,
            vertical_alignment: true,
            ..FormatFields::default()
        }
        .names(),
        vec!["underline", "fontFamily", "fontSize", "verticalAlignment"]
    );
    assert_eq!(
        FormatFields {
            background: true,
            value: true,
            ..FormatFields::default()
        }
        .names(),
        vec!["background", "value"]
    );
}

#[test]
fn mock_connector_reads_a_bold_filled_header() {
    let conn = demo_db();
    let registry = ConnectorRegistry::with_default_connectors();
    let fields = FormatFields {
        background: true,
        bold: true,
        value: true,
        ..FormatFields::default()
    };
    let grid = registry
        .read_formats(
            &conn,
            DEMO_SOURCE_ID,
            DEMO_TABLE_ID,
            &request(None, fields, READ_FORMATS_MAX_CELLS),
        )
        .unwrap();
    assert!(grid.rows >= 2 && grid.columns >= 1);
    let output = encode_formats(&grid, &fields, FormatSource::Effective);
    let background = output.background.unwrap();
    assert_eq!(background.palette, vec![None, Some("#f3f4f6".to_string())]);
    assert_eq!(background.grid[0], format!("1*{}", grid.columns));
    assert_eq!(background.grid[1], "");
    assert_eq!(output.value.unwrap().grid[0][0], "Name");

    // Background only: the unformatted data rows are trimmed away.
    let header_only = registry
        .read_formats(
            &conn,
            DEMO_SOURCE_ID,
            DEMO_TABLE_ID,
            &request(None, background_only(), READ_FORMATS_MAX_CELLS),
        )
        .unwrap();
    assert_eq!(header_only.rows, 1);

    let window = registry
        .read_formats(
            &conn,
            DEMO_SOURCE_ID,
            DEMO_TABLE_ID,
            &request(Some("B1:C2"), fields, READ_FORMATS_MAX_CELLS),
        )
        .unwrap();
    assert_eq!((window.rows, window.columns), (2, 2));
    assert_eq!(window.cells[0][0].value, "Email");
}
