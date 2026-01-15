select
  map_name,
  id,
  url,
  type
from wad_map_images
where wad_id = $1::uuid
order by map_name, coalesce(type, ''), url, id;
