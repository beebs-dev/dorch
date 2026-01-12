insert into wad_map_monsters (wad_id, map_name, monster, count)
values ($1::uuid, $2::text, $3::text, $4::bigint::int);
