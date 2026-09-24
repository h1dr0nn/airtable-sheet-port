use super::*;

fn json_map() -> JsonMap {
    JsonMap::new()
}

#[test]
fn rejects_empty_source_id() {
    let args = ListTablesArgs {
        source_id: Some(String::new()),
    };
    let error = args.validate().expect_err("empty sourceId must fail");
    assert_eq!(error.to_string(), "sourceId must be a non-empty string");
}

#[test]
fn source_id_is_optional_but_never_empty() {
    let omitted = SourceTableArgs {
        source_id: None,
        table_id: "t".to_string(),
    };
    assert!(
        omitted.validate().is_ok(),
        "an omitted sourceId auto-routes"
    );

    let missing_table = SourceTableArgs {
        source_id: None,
        table_id: String::new(),
    };
    assert_eq!(
        missing_table
            .validate()
            .expect_err("tableId is required")
            .to_string(),
        "tableId must be a non-empty string"
    );

    let parsed: ListTablesArgs = serde_json::from_str("{}").expect("sourceId may be absent");
    assert!(parsed.source_id.is_none());
    assert!(parsed.validate().is_ok());
}

#[test]
fn read_cells_parses_an_optional_range() {
    let base = |range: Option<&str>| ReadCellsArgs {
        source_id: None,
        table_id: "t".to_string(),
        range: range.map(str::to_string),
        limit: None,
        offset: None,
    };
    let (limit, offset, range) = base(None).validate().expect("no range");
    assert_eq!((limit, offset), (100, 0));
    assert!(range.is_none());

    let (_, _, range) = base(Some("B40:F60")).validate().expect("block range");
    let range = range.expect("parsed range");
    assert_eq!((range.start_col, range.start_row), (Some(1), Some(39)));
    assert!(base(Some("A:C")).validate().is_ok());
    assert!(base(Some("5:9")).validate().is_ok());

    let qualified = base(Some("Sheet1!A1:B2"))
        .validate()
        .expect_err("a sheet-qualified range is refused");
    assert!(qualified.to_string().contains("tableId"), "{qualified}");
    assert_eq!(
        base(Some(""))
            .validate()
            .expect_err("empty range")
            .to_string(),
        "range must be a non-empty string"
    );
    assert!(base(Some("B40-F60")).validate().is_err());
}

#[test]
fn delete_sheet_needs_confirm() {
    let base = |confirm| DeleteSheetArgs {
        source_id: None,
        table_id: "t".to_string(),
        confirm,
        dry_run: false,
    };
    assert_eq!(
        base(false).validate().expect_err("no confirm").to_string(),
        "delete_sheet needs confirm: true"
    );
    assert!(base(true).validate().is_ok());
    let parsed: DeleteSheetArgs =
        serde_json::from_str(r#"{"tableId":"t"}"#).expect("confirm defaults to false");
    assert!(parsed.validate().is_err());
}

#[test]
fn dry_run_defaults_to_false() {
    let parsed: CreateSpreadsheetArgs =
        serde_json::from_str(r#"{"title":"Budget"}"#).expect("parses");
    assert!(!parsed.dry_run);
    let parsed: CreateSpreadsheetArgs =
        serde_json::from_str(r#"{"title":"Budget","dryRun":true}"#).expect("parses");
    assert!(parsed.dry_run);
}

#[test]
fn read_table_applies_defaults() {
    let args = ReadTableArgs {
        source_id: Some("mock-source".to_string()),
        table_id: "customers".to_string(),
        limit: None,
        offset: None,
    };
    assert_eq!(args.validate().expect("defaults valid"), (100, 0));
}

#[test]
fn read_table_rejects_out_of_range_limit_and_offset() {
    let base = |limit, offset| ReadTableArgs {
        source_id: Some("s".to_string()),
        table_id: "t".to_string(),
        limit,
        offset,
    };
    assert!(base(Some(0), None).validate().is_err(), "limit below 1");
    assert!(base(Some(501), None).validate().is_err(), "limit above 500");
    assert!(base(Some(-1), None).validate().is_err(), "negative limit");
    assert!(base(None, Some(-1)).validate().is_err(), "negative offset");
    assert_eq!(
        base(Some(500), Some(7)).validate().expect("bounds valid"),
        (500, 7)
    );
    assert_eq!(
        base(Some(1), Some(0)).validate().expect("minimums valid"),
        (1, 0)
    );
}

#[test]
fn find_records_enforces_query_length() {
    let base = |query: String| FindRecordsArgs {
        source_id: Some("s".to_string()),
        table_id: "t".to_string(),
        query,
    };
    assert!(base(String::new()).validate().is_err(), "empty query");
    assert!(base("x".repeat(201)).validate().is_err(), "201 chars");
    assert!(base("x".repeat(200)).validate().is_ok(), "200 chars fits");
}

#[test]
fn update_records_enforces_patch_bounds_and_record_ids() {
    let patch = |record_id: &str| PatchArg {
        record_id: record_id.to_string(),
        fields: json_map(),
    };
    let base = |patches| UpdateRecordsArgs {
        source_id: Some("s".to_string()),
        table_id: "t".to_string(),
        patches,
        dry_run: false,
    };
    assert!(base(Vec::new()).validate().is_err(), "no patches");
    let too_many: Vec<PatchArg> = (0..101).map(|_| patch("rec")).collect();
    assert!(base(too_many).validate().is_err(), "101 patches");
    let error = base(vec![patch("")])
        .validate()
        .expect_err("empty recordId must fail");
    assert_eq!(
        error.to_string(),
        "patches[0].recordId must be a non-empty string"
    );
    assert!(base(vec![patch("rec_1")]).validate().is_ok());
}

#[test]
fn append_records_enforces_batch_bounds() {
    let base = |records| AppendRecordsArgs {
        source_id: Some("s".to_string()),
        table_id: "t".to_string(),
        records,
        format: FormatSpec::default(),
        dry_run: false,
    };
    assert!(base(Vec::new()).validate().is_err(), "no records");
    let too_many: Vec<JsonMap> = (0..101).map(|_| json_map()).collect();
    assert!(base(too_many).validate().is_err(), "101 records");
    assert!(base(vec![json_map()]).validate().is_ok());
}

#[test]
fn append_records_carries_a_bundled_format_plan_when_present() {
    let args = AppendRecordsArgs {
        source_id: Some("s".to_string()),
        table_id: "t".to_string(),
        records: vec![json_map()],
        format: FormatSpec {
            freeze_rows: Some(1),
            ..FormatSpec::default()
        },
        dry_run: false,
    };
    let plan = args.validate().expect("valid").expect("plan present");
    assert_eq!(plan.freeze_rows, Some(1));

    let plain = AppendRecordsArgs {
        source_id: Some("s".to_string()),
        table_id: "t".to_string(),
        records: vec![json_map()],
        format: FormatSpec::default(),
        dry_run: false,
    };
    assert!(
        plain.validate().expect("valid").is_none(),
        "a plain append carries no plan"
    );
}

#[test]
fn update_cells_parses_refs_and_rejects_bad_ones() {
    let base = |cells| UpdateCellsArgs {
        source_id: Some("s".to_string()),
        table_id: "t".to_string(),
        cells,
        dry_run: false,
    };
    let write = |cell: &str| CellWriteArg {
        cell: cell.to_string(),
        value: "350h".to_string(),
    };

    let cells = base(vec![write("e48"), write("AA100")])
        .validate()
        .expect("valid refs");
    assert_eq!(cells[0].column, "E", "letters normalize to uppercase");
    assert_eq!(cells[0].row, 48);
    assert_eq!(cells[0].a1(), "E48");
    assert_eq!(cells[1].column, "AA");

    assert!(base(Vec::new()).validate().is_err(), "no cells");
    assert!(base(vec![write("48")]).validate().is_err(), "no column");
    assert!(base(vec![write("E")]).validate().is_err(), "no row");
    assert!(base(vec![write("E0")]).validate().is_err(), "row below 1");
    assert!(
        base(vec![write("AAA1")]).validate().is_err(),
        "beyond the ZZ column window"
    );
}

#[test]
fn commit_change_requires_at_least_one_id() {
    let neither = CommitChangeArgs {
        change_id: None,
        change_ids: None,
    };
    assert_eq!(
        neither
            .ids()
            .expect_err("neither id form given")
            .to_string(),
        "provide changeId (single) or changeIds (batch)"
    );

    let empty = CommitChangeArgs {
        change_id: Some(String::new()),
        change_ids: None,
    };
    assert_eq!(
        empty.ids().expect_err("empty changeId").to_string(),
        "changeId must be a non-empty string"
    );

    let single = CommitChangeArgs {
        change_id: Some("chg_1".to_string()),
        change_ids: None,
    };
    assert_eq!(single.ids().expect("valid"), vec!["chg_1".to_string()]);
    assert!(!single.is_batch(), "singular form is not a batch");

    let batch = CommitChangeArgs {
        change_id: None,
        change_ids: Some(vec!["chg_1".to_string(), "chg_2".to_string()]),
    };
    assert_eq!(batch.ids().expect("valid").len(), 2);
    assert!(batch.is_batch(), "plural form is a batch");
}

#[test]
fn get_audit_log_applies_default_and_bounds() {
    let base = |limit| GetAuditLogArgs { limit };
    assert_eq!(base(None).validate().expect("default"), 100);
    assert_eq!(base(Some(500)).validate().expect("max"), 500);
    assert!(base(Some(0)).validate().is_err(), "limit below 1");
    assert!(base(Some(501)).validate().is_err(), "limit above 500");
}

fn empty_spec() -> FormatSpec {
    FormatSpec {
        formats: Vec::new(),
        freeze_rows: None,
        freeze_columns: None,
        column_widths: Vec::new(),
    }
}

fn cell_format_arg(range: &str) -> CellFormatArg {
    CellFormatArg {
        range: range.to_string(),
        bold: None,
        italic: None,
        font_size: None,
        font_color: None,
        background_color: None,
        horizontal_alignment: None,
        number_format: None,
        number_format_type: None,
        wrap: None,
        border: None,
    }
}

#[test]
fn format_spec_rejects_a_plan_that_changes_nothing() {
    assert!(empty_spec().to_plan().is_err(), "an empty plan is rejected");
}

#[test]
fn format_table_args_validate_ids_then_delegate() {
    let missing_id = FormatTableArgs {
        source_id: Some(String::new()),
        table_id: "t".to_string(),
        format: FormatSpec {
            freeze_rows: Some(1),
            ..empty_spec()
        },
        dry_run: false,
    };
    assert_eq!(
        missing_id
            .validate()
            .expect_err("empty sourceId")
            .to_string(),
        "sourceId must be a non-empty string"
    );

    let ok = FormatTableArgs {
        source_id: Some("s".to_string()),
        table_id: "t".to_string(),
        format: FormatSpec {
            freeze_rows: Some(1),
            ..empty_spec()
        },
        dry_run: false,
    };
    assert_eq!(ok.validate().expect("valid").freeze_rows, Some(1));
}

#[test]
fn format_spec_accepts_a_freeze_only_plan() {
    let plan = FormatSpec {
        freeze_rows: Some(1),
        ..empty_spec()
    }
    .to_plan()
    .expect("freeze-only plan is valid");
    assert_eq!(plan.freeze_rows, Some(1));
    assert!(plan.formats.is_empty());
}

#[test]
fn format_spec_validates_colors_alignment_border_and_range() {
    let good = FormatSpec {
        formats: vec![CellFormatArg {
            bold: Some(true),
            background_color: Some("#F3F4F6".to_string()),
            horizontal_alignment: Some("center".to_string()),
            border: Some("BOTTOM".to_string()),
            ..cell_format_arg("A1:D1")
        }],
        ..empty_spec()
    };
    let plan = good.to_plan().expect("valid formatting");
    let format = &plan.formats[0];
    assert_eq!(
        format.background_color.as_deref(),
        Some("#f3f4f6"),
        "hex is normalized to lowercase"
    );
    assert_eq!(
        format.horizontal_alignment,
        Some(HorizontalAlignment::Center)
    );
    assert_eq!(format.border, Some(BorderStyle::Bottom));

    let bad_color = FormatSpec {
        formats: vec![CellFormatArg {
            background_color: Some("f3f4f6".to_string()),
            ..cell_format_arg("A1")
        }],
        ..empty_spec()
    };
    assert!(bad_color.to_plan().is_err(), "missing # is rejected");

    let bad_align = FormatSpec {
        formats: vec![CellFormatArg {
            horizontal_alignment: Some("justify".to_string()),
            ..cell_format_arg("A1")
        }],
        ..empty_spec()
    };
    assert!(
        bad_align.to_plan().is_err(),
        "unknown alignment is rejected"
    );

    let bad_range = FormatSpec {
        formats: vec![cell_format_arg("not-a-range")],
        ..empty_spec()
    };
    assert!(bad_range.to_plan().is_err(), "a bad A1 range is rejected");
}

#[test]
fn format_spec_enforces_numeric_bounds() {
    let bad_freeze = FormatSpec {
        freeze_rows: Some(101),
        ..empty_spec()
    };
    assert!(bad_freeze.to_plan().is_err(), "freeze above the cap");

    let bad_font = FormatSpec {
        formats: vec![CellFormatArg {
            font_size: Some(0),
            ..cell_format_arg("A1")
        }],
        ..empty_spec()
    };
    assert!(bad_font.to_plan().is_err(), "font size below the minimum");

    let bad_width = FormatSpec {
        column_widths: vec![ColumnWidthArg {
            column: "A".to_string(),
            pixels: 1,
        }],
        ..empty_spec()
    };
    assert!(bad_width.to_plan().is_err(), "width below the minimum");

    let ok_width = FormatSpec {
        column_widths: vec![ColumnWidthArg {
            column: "A".to_string(),
            pixels: 160,
        }],
        ..empty_spec()
    };
    assert!(ok_width.to_plan().is_ok());
}
