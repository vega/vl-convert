//! fontdb integration for Google Fonts batches.
//!
//! Available when the `fontdb` feature is enabled.

use crate::types::LoadedFontBatch;
use std::sync::Arc;
use tinyvec::TinyVec;

/// A batch of font faces registered in a `fontdb::Database`.
#[derive(Debug, Clone)]
pub struct RegisteredFontBatch {
    per_source_ids: Vec<TinyVec<[fontdb::ID; 8]>>,
    all_ids: Vec<fontdb::ID>,
}

impl RegisteredFontBatch {
    pub fn per_source_ids(&self) -> &[TinyVec<[fontdb::ID; 8]>] {
        &self.per_source_ids
    }

    pub fn face_ids(&self) -> &[fontdb::ID] {
        &self.all_ids
    }

    pub(crate) fn into_face_ids(self) -> Vec<fontdb::ID> {
        self.all_ids
    }
}

/// Extension trait for registering/unregistering Google Fonts batches
/// with a `fontdb::Database`.
pub trait GoogleFontsDatabaseExt {
    /// Register a `LoadedFontBatch`, returning a `RegisteredFontBatch`
    /// with per-source ID tracking.
    fn register_google_fonts_batch(&mut self, batch: LoadedFontBatch) -> RegisteredFontBatch;

    /// Unregister all faces from a `RegisteredFontBatch`.
    fn unregister_google_fonts_batch(&mut self, registration: RegisteredFontBatch);
}

impl GoogleFontsDatabaseExt for fontdb::Database {
    fn register_google_fonts_batch(&mut self, batch: LoadedFontBatch) -> RegisteredFontBatch {
        let mut per_source_ids = Vec::new();
        let mut all_ids = Vec::new();

        for data in batch.font_data {
            let source = fontdb::Source::Binary(data as Arc<dyn AsRef<[u8]> + Send + Sync>);
            let ids = self.load_font_source(source);
            all_ids.extend(ids.iter().copied());
            per_source_ids.push(ids);
        }

        RegisteredFontBatch {
            per_source_ids,
            all_ids,
        }
    }

    fn unregister_google_fonts_batch(&mut self, registration: RegisteredFontBatch) {
        for id in registration.into_face_ids() {
            self.remove_face(id);
        }
    }
}
