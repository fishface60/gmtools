// The GCS data structure has most items be a subclass of ListRow.
// The serialisation order of keys intersperses those that come from the row
// and those that come from the subclass.
// Rust doesn't have subclasses to imitate the model,
// and it's harder to see at a glance than the decorated structure form
// so it's more convenient to provide common structures to embed.
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Comes after type and version but before the "saveSelf" subclass section.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RowIdFragment {
    pub id: Uuid,

    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub based_on_id: Option<Uuid>,
    // Is a base64-encoded SHA3-256 hash of the object
    // as stored as unindented compact JSON,
    // omitting the open state
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub based_on_hash: Option<String>,
}
