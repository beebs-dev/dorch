select
  count(*) over() as full_count,
  w.wad_id,
  w.meta_json
from wads w
where exists (select 1 from wad_map_images i where i.wad_id = w.wad_id)
order by
  (nullif(btrim(w.title), '') is not null) desc,
  random()
limit $1;
