select
  map_json
from wad_maps
where wad_id = $1::uuid
order by map_name asc;
