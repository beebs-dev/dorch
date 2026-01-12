insert into wad_text_files (wad_id, source, name, contents, ord)
values ($1::uuid, $2::text, $3::text, $4::text, $5::int);
