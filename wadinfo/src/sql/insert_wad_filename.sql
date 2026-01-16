-- Insert one filename row for wad_filenames (meta.filenames / filenames.json).
insert into wad_filenames (wad_id, filename, ord)
values ($1::uuid, $2::text, $3::int);
