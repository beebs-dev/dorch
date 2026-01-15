insert into wad_map_images (wad_id, map_name, url, type)
values ($1::uuid, $2::text, $3::text, $4::text);
