insert into wad_descriptions (wad_id, description, ord)
values ($1::uuid, $2::text, $3::int);
