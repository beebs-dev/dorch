select
  map_json
from wad_maps
where wad_id = $1::uuid
  and map_name = $2::text
limit 1;
