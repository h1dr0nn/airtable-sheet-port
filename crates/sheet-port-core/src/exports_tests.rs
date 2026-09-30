use super::*;

fn temp_dir() -> PathBuf {
    std::env::temp_dir()
        .join("sheet-port-exports-tests")
        .join(uuid::Uuid::new_v4().to_string())
}

#[test]
fn exports_dir_sits_beside_the_database() {
    let db = Path::new("some").join("dir").join("sheet-port.db");
    assert_eq!(
        exports_dir_for(&db),
        Path::new("some").join("dir").join("exports")
    );
    assert_eq!(exports_dir_for(Path::new("x.db")), PathBuf::from("exports"));
}

#[test]
fn plain_json_file_names_are_accepted() {
    for name in ["a.json", "levels.json", "L1-5_tab.v2.json", "A.json"] {
        assert!(validate_export_name(name).is_ok(), "{name}");
    }
    let long = format!("{}.json", "a".repeat(EXPORT_NAME_MAX_LEN - 5));
    assert!(validate_export_name(&long).is_ok());
}

#[test]
fn paths_traversal_and_odd_names_are_rejected() {
    let too_long = format!("{}.json", "a".repeat(EXPORT_NAME_MAX_LEN));
    for name in [
        "",
        "../x.json",
        "..\\x.json",
        "a/b.json",
        "a\\b.json",
        "/abs.json",
        "C:x.json",
        "a..json",
        "..json",
        ".json",
        "x.txt",
        "x.json.txt",
        "x json.json",
        "é.json",
        "NUL.json",
        "con.json",
        "com1.data.json",
        too_long.as_str(),
    ] {
        let error = validate_export_name(name).expect_err(name);
        assert!(
            matches!(error, CoreError::InvalidInput(_)),
            "{name}: {error:?}"
        );
    }
}

#[test]
fn export_path_is_a_direct_child_of_the_directory() {
    let dir = Path::new("base").join("exports");
    assert_eq!(export_path(&dir, "a.json").unwrap(), dir.join("a.json"));
    assert!(export_path(&dir, "../a.json").is_err());
}

#[test]
fn write_export_creates_the_directory_and_overwrites() {
    let dir = temp_dir();
    let first = write_export(&dir, "out.json", b"{\"a\":1}").unwrap();
    assert!(first.is_absolute());
    assert_eq!(first.parent().unwrap(), std::path::absolute(&dir).unwrap());
    let second = write_export(&dir, "out.json", b"{}").unwrap();
    assert_eq!(first, second);
    assert_eq!(std::fs::read(&second).unwrap(), b"{}");
    assert!(write_export(&dir, "../out.json", b"{}").is_err());
    let _ = std::fs::remove_dir_all(&dir);
}
