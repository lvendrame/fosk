use serde::{Deserialize, Serialize};

/// Strategy used for generating or interpreting document IDs in a collection.
///
/// - `Uuid`: use UUID strings for the document id.
/// - `Int`: use incrementing integers for the document id.
/// - `None`: no automatic id generation; caller must provide an id in the document.
#[derive(Debug, Default, Clone, Copy, PartialEq, Deserialize, Serialize)]
pub enum IdType {
    /// Use UUID string values as IDs (default).
    #[default]
    Uuid,
    /// Use integer IDs generated sequentially.
    Int,
    /// No automatic id generation; documents must include the configured id key.
    None,
}

