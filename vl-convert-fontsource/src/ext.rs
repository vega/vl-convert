use crate::types::{LoadedFontBatch, RegisteredFontBatch};

pub trait FontsourceDatabaseExt {
    fn register_fontsource_batch(&mut self, batch: LoadedFontBatch) -> RegisteredFontBatch;

    fn unregister_fontsource_batch(&mut self, registration: RegisteredFontBatch);
}

impl FontsourceDatabaseExt for fontdb::Database {
    fn register_fontsource_batch(&mut self, batch: LoadedFontBatch) -> RegisteredFontBatch {
        let mut per_source_ids = Vec::new();
        let mut all_ids = Vec::new();

        for source in batch.into_sources() {
            let ids = self.load_font_source(source);
            all_ids.extend(ids.iter().copied());
            per_source_ids.push(ids);
        }

        RegisteredFontBatch::new(per_source_ids, all_ids)
    }

    fn unregister_fontsource_batch(&mut self, registration: RegisteredFontBatch) {
        for id in registration.into_face_ids() {
            // remove_face is a best-effort no-op for missing IDs.
            self.remove_face(id);
        }
    }
}
