delete from wad_map_images
where wad_id = $1::uuid
  and map_name = $2::text;
