insert into wad_map_list (wad_id, map_name, ord)
values ($1::uuid, $2::text, $3::int);
