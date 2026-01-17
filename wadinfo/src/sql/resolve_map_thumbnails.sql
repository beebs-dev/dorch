-- Resolve one thumbnail per (wad_id, map_name).
-- Excludes pano renders.
--
-- Inputs are two parallel arrays (same length): uuid[] and text[].

with input as (
  select *
  from unnest($1::uuid[], $2::text[]) as t(wad_id, map_name)
)
select distinct on (i.wad_id, i.map_name)
  i.wad_id,
  i.map_name,
  wmi.url
from input i
join wad_map_images wmi
  on wmi.wad_id = i.wad_id
 and wmi.map_name = i.map_name
where coalesce(wmi.type, '') <> 'pano'
order by
  i.wad_id,
  i.map_name,
  coalesce(wmi.type, ''),
  wmi.url,
  wmi.id;
