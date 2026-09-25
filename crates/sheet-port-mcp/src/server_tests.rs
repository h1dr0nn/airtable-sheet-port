use serde_json::Value;

use super::*;

/// The `inputSchema` a client receives in `tools/list`, as JSON.
fn input_schema(tool_name: &str) -> Value {
    let tool = SheetPortServer::tool_router()
        .list_all()
        .into_iter()
        .find(|tool| tool.name == tool_name)
        .unwrap_or_else(|| panic!("tool {tool_name} is registered"));
    serde_json::to_value(&*tool.input_schema).expect("schema serializes")
}

/// The item schema of an array property, which must describe its fields.
fn item_properties<'a>(schema: &'a Value, property: &str) -> &'a serde_json::Map<String, Value> {
    let items = &schema["properties"][property]["items"];
    let properties = items["properties"]
        .as_object()
        .unwrap_or_else(|| panic!("{property} items have no properties: {items}"));
    assert!(!properties.is_empty(), "{property} items are {{}}");
    properties
}

#[test]
fn format_table_schema_lists_validations_and_conditional_formats_inline() {
    let schema = input_schema("format_table");
    let properties = schema["properties"].as_object().expect("properties");
    for key in [
        "tableId",
        "formats",
        "freezeRows",
        "freezeColumns",
        "columnWidths",
        "validations",
        "conditionalFormats",
        "replaceIntersecting",
        "dryRun",
    ] {
        assert!(properties.contains_key(key), "format_table lacks {key}");
    }
    for key in ["validations", "conditionalFormats"] {
        assert!(
            properties[key]["description"].is_string(),
            "{key} has a description"
        );
    }

    let validation = item_properties(&schema, "validations");
    for key in ["range", "type", "values", "strict", "showDropdown"] {
        assert!(validation.contains_key(key), "validations items lack {key}");
    }
    let rule = item_properties(&schema, "conditionalFormats");
    for key in ["range", "when", "backgroundColor", "fontColor", "bold"] {
        assert!(
            rule.contains_key(key),
            "conditionalFormats items lack {key}"
        );
    }
    let when = rule["when"]["properties"]
        .as_object()
        .expect("when inlined");
    assert!(when.contains_key("textEq") && when.contains_key("formula"));
    let format = item_properties(&schema, "formats");
    assert!(format.contains_key("range") && format.contains_key("numberFormat"));
    item_properties(&schema, "columnWidths");

    let text = schema.to_string();
    assert!(
        !text.contains("$ref") && !text.contains("$defs"),
        "schema is fully inlined: {text}"
    );
}

#[test]
fn append_records_schema_carries_the_format_fields() {
    let schema = input_schema("append_records");
    let properties = schema["properties"].as_object().expect("properties");
    for key in [
        "records",
        "formats",
        "validations",
        "conditionalFormats",
        "replaceIntersecting",
    ] {
        assert!(properties.contains_key(key), "append_records lacks {key}");
    }
    item_properties(&schema, "validations");
    item_properties(&schema, "conditionalFormats");
}

#[test]
fn every_tool_schema_is_inline_with_described_array_items() {
    for tool in SheetPortServer::tool_router().list_all() {
        let schema = serde_json::to_value(&*tool.input_schema).expect("schema");
        assert_eq!(schema["type"], "object", "{} root type", tool.name);
        let text = schema.to_string();
        assert!(
            !text.contains("$ref") && !text.contains("$defs"),
            "{} schema has refs: {text}",
            tool.name
        );
        let Some(properties) = schema["properties"].as_object() else {
            continue;
        };
        for (key, property) in properties {
            let items = &property["items"];
            if items["type"] == "object" {
                assert!(
                    items["properties"]
                        .as_object()
                        .is_some_and(|fields| !fields.is_empty())
                        || items.get("additionalProperties").is_some(),
                    "{}.{key} items are opaque: {items}",
                    tool.name
                );
            }
        }
    }
    let update_cells = input_schema("update_cells");
    let cells = item_properties(&update_cells, "cells");
    assert!(cells.contains_key("cell") && cells.contains_key("value"));
    let update_records = input_schema("update_records");
    let patches = item_properties(&update_records, "patches");
    assert!(patches.contains_key("recordId") && patches.contains_key("fields"));
}

#[test]
fn get_table_style_schema_offers_header_row() {
    let schema = input_schema("get_table_style");
    assert!(schema["properties"]["headerRow"]["description"].is_string());
    assert_eq!(schema["required"], serde_json::json!(["tableId"]));
}

#[test]
fn client_identity_trims_caps_and_drops_blank_values() {
    let identity = client_identity("  claude-code ", "2.1.0");
    assert_eq!(identity.client_name.as_deref(), Some("claude-code"));
    assert_eq!(identity.client_version.as_deref(), Some("2.1.0"));
    assert_eq!(identity.exe_path, None, "exe paths are set at startup only");

    let blank = client_identity("   ", "");
    assert_eq!(blank.client_name, None);
    assert_eq!(blank.client_version, None);

    let long = client_identity(&"x".repeat(500), "1");
    assert_eq!(long.client_name.map(|name| name.len()), Some(128));
}
