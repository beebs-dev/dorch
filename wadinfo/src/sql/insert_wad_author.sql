insert into wad_authors (wad_id, author, ord)
values ($1::uuid, $2::text, $3::int);
