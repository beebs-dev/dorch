insert into wad_map_textures (wad_id, map_name, texture, count)
values ($1::uuid, $2::text, $3::text, $4::int);
