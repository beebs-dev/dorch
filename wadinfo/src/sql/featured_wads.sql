select
  count(*) over() as full_count,
  w.wad_id,
  w.meta_json
from wads w
where exists (select 1 from wad_map_images i where i.wad_id = w.wad_id)
order by random()
limit $1;
