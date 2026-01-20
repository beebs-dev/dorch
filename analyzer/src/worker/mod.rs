use dorch_wadinfo::client::ReadWad;

pub mod analyzer;
pub mod app;
pub mod map;
pub mod wad;

pub fn optimize_readwad(input: &mut ReadWad) {
    //input.meta.meta.id = Uuid::nil(); // do this later (as necessary)
    input.meta.meta.sha1 = String::new();
    input.meta.meta.sha256 = None;
    input.meta.analysis = None; // avoid self-reference
    for map in &mut input.maps {
        map.analysis = None;
        map.images.clear();
    }
    if let Some(text_files) = &mut input.meta.text_files {
        for text_file in text_files {
            // truncate at character boundaries
            if let Some(truncated) = text_file
                .contents
                .char_indices()
                .nth(5_000)
                .map(|(idx, _)| &text_file.contents[..idx])
            {
                text_file.contents = format!(
                    "{}\n\nFile was truncated due to length. Original size in bytes: {}",
                    truncated,
                    text_file.contents.len()
                );
            }
        }
    }
}
