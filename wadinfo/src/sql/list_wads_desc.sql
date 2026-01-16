select
  count(*) over() as full_count,
  w.wad_id,
  w.meta_json
from wads w
order by
  exists (select 1 from wad_map_images i where i.wad_id = w.wad_id) desc,
  lower(coalesce(w.title, '')) desc,
  w.wad_id desc
offset $1
limit $2;
