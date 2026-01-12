insert into wad_map_items (wad_id, map_name, item, count)
values ($1::uuid, $2::text, $3::text, $4::bigint::int);
