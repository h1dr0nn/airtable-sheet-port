//! Tool input models and bounds validation (docs/mcp-tools.md). Deserializing
//! covers types; every numeric/length bound is enforced here manually so an
//! out-of-range value surfaces as a tool error with a clear message instead
//! of a raw schema failure.

use schemars::JsonSchema;
use serde::Deserialize;
use sheet_port_core::constants::{
    AUDIT_LIMIT_DEFAULT, AUDIT_LIMIT_MAX, FIND_QUERY_MAX_LEN, READ_LIMIT_DEFAULT, READ_LIMIT_MAX,
    READ_LIMIT_MIN, WRITE_BATCH_MAX,
};
use sheet_port_core::types::JsonMap;
use sheet_port_core::CoreError;

/// Matches the audit module's own lower bound (kept private there).
const AUDIT_LIMIT_MIN: i64 = 1;
const FIND_QUERY_MIN_LEN: usize = 1;
const WRITE_BATCH_MIN: usize = 1;

fn invalid(message: String) -> CoreError {
    CoreError::InvalidInput(message)
}

fn require_non_empty(value: &str, field: &str) -> Result<(), CoreError> {
    if value.is_empty() {
        return Err(invalid(format!("{field} must be a non-empty string")));
    }
    Ok(())
}

fn require_batch_size(len: usize, field: &str) -> Result<(), CoreError> {
    if !(WRITE_BATCH_MIN..=WRITE_BATCH_MAX).contains(&len) {
        return Err(invalid(format!(
            "{field} must contain between {WRITE_BATCH_MIN} and {WRITE_BATCH_MAX} items"
        )));
    }
    Ok(())
}

fn bounded_limit(limit: Option<i64>, default: i64, min: i64, max: i64) -> Result<i64, CoreError> {
    let limit = limit.unwrap_or(default);
    if !(min..=max).contains(&limit) {
        return Err(invalid(format!(
            "limit must be an integer between {min} and {max}"
        )));
    }
    Ok(limit)
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListTablesArgs {
    pub source_id: String,
}

impl ListTablesArgs {
    pub fn validate(&self) -> Result<(), CoreError> {
        require_non_empty(&self.source_id, "sourceId")
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SourceTableArgs {
    pub source_id: String,
    pub table_id: String,
}

impl SourceTableArgs {
    pub fn validate(&self) -> Result<(), CoreError> {
        require_non_empty(&self.source_id, "sourceId")?;
        require_non_empty(&self.table_id, "tableId")
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReadTableArgs {
    pub source_id: String,
    pub table_id: String,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
}

impl ReadTableArgs {
    /// Returns the effective `(limit, offset)` after defaults and bounds.
    pub fn validate(&self) -> Result<(i64, i64), CoreError> {
        require_non_empty(&self.source_id, "sourceId")?;
        require_non_empty(&self.table_id, "tableId")?;
        let limit = bounded_limit(
            self.limit,
            READ_LIMIT_DEFAULT,
            READ_LIMIT_MIN,
            READ_LIMIT_MAX,
        )?;
        let offset = self.offset.unwrap_or(0);
        if offset < 0 {
            return Err(invalid("offset must be an integer >= 0".to_string()));
        }
        Ok((limit, offset))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FindRecordsArgs {
    pub source_id: String,
    pub table_id: String,
    pub query: String,
}

impl FindRecordsArgs {
    pub fn validate(&self) -> Result<(), CoreError> {
        require_non_empty(&self.source_id, "sourceId")?;
        require_non_empty(&self.table_id, "tableId")?;
        let length = self.query.chars().count();
        if !(FIND_QUERY_MIN_LEN..=FIND_QUERY_MAX_LEN).contains(&length) {
            return Err(invalid(format!(
                "query must be between {FIND_QUERY_MIN_LEN} and {FIND_QUERY_MAX_LEN} characters"
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PatchArg {
    pub record_id: String,
    pub fields: JsonMap,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PreviewUpdateArgs {
    pub source_id: String,
    pub table_id: String,
    pub patches: Vec<PatchArg>,
}

impl PreviewUpdateArgs {
    pub fn validate(&self) -> Result<(), CoreError> {
        require_non_empty(&self.source_id, "sourceId")?;
        require_non_empty(&self.table_id, "tableId")?;
        require_batch_size(self.patches.len(), "patches")?;
        for (index, patch) in self.patches.iter().enumerate() {
            require_non_empty(&patch.record_id, &format!("patches[{index}].recordId"))?;
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AppendRecordsArgs {
    pub source_id: String,
    pub table_id: String,
    pub records: Vec<JsonMap>,
}

impl AppendRecordsArgs {
    pub fn validate(&self) -> Result<(), CoreError> {
        require_non_empty(&self.source_id, "sourceId")?;
        require_non_empty(&self.table_id, "tableId")?;
        require_batch_size(self.records.len(), "records")
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CommitChangeArgs {
    pub change_id: String,
}

impl CommitChangeArgs {
    pub fn validate(&self) -> Result<(), CoreError> {
        require_non_empty(&self.change_id, "changeId")
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GetAuditLogArgs {
    #[serde(default)]
    pub limit: Option<i64>,
}

impl GetAuditLogArgs {
    /// Returns the effective limit after the default and bounds.
    pub fn validate(&self) -> Result<i64, CoreError> {
        bounded_limit(
            self.limit,
            AUDIT_LIMIT_DEFAULT,
            AUDIT_LIMIT_MIN,
            AUDIT_LIMIT_MAX,
        )
    }
}

#[cfg(test)]
#[path = "args_tests.rs"]
mod tests;
