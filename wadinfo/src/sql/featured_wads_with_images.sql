with featured as (
  select
    w.wad_id,
    w.meta_json,
    (nullif(trim(w.meta_json->>'title'), '') is null) as title_missing,
    random() as rnd
  from wads w
  where w.hidden = false
    and w.can_download = true
    and exists (select 1 from wad_map_images i where i.wad_id = w.wad_id)
    and exists (select 1 from wad_analysis a where a.wad_id = w.wad_id)
  order by
    title_missing asc,
    rnd
  limit $1
)
select
  f.wad_id,
  f.meta_json,
  coalesce(
    jsonb_agg(
      jsonb_build_object(
        'id', i.id,
        'url', i.url,
        'type', i.type
      )
      order by i.map_name, i.id
    ) filter (where coalesce(i.type, '') <> 'pano'),
    '[]'::jsonb
  ) as images_json
from featured f
left join wad_map_images i
  on i.wad_id = f.wad_id
group by f.title_missing, f.rnd, f.wad_id, f.meta_json
order by f.title_missing asc, f.rnd;
