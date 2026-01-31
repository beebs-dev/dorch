with
params as (
  -- Rotate the "random" ordering over time without paying the full cost of
  -- ORDER BY random() across a large dataset.
  --
  -- This is deterministic within a given 15-second window, but shifts every 15
  -- seconds, and the
  -- wrap-around logic ensures the whole dataset is eventually covered.
  select md5(((extract(epoch from now())::bigint / 15))::text) as threshold
),
eligible as (
  select
    w.wad_id,
    w.meta_json,
    (nullif(trim(w.meta_json->>'title'), '') is null) as title_missing,
    md5(w.wad_id::text) as shuffle
  from wads w
  where w.hidden = false
    and w.can_download = true
    and exists (select 1 from wad_map_images i where i.wad_id = w.wad_id)
    and exists (select 1 from wad_analysis a where a.wad_id = w.wad_id)
),
title_present as (
  select *
  from (
    (select e.*
     from eligible e, params p
     where e.title_missing = false and e.shuffle >= p.threshold
     order by e.shuffle
     limit $1)
    union all
    (select e.*
     from eligible e, params p
     where e.title_missing = false and e.shuffle < p.threshold
     order by e.shuffle
     limit $1)
  ) q
  limit $1
),
title_present_count as (
  select count(*)::int as n from title_present
),
title_missing as (
  select *
  from (
    (select e.*
     from eligible e, params p
     where e.title_missing = true and e.shuffle >= p.threshold
     order by e.shuffle
     limit $1)
    union all
    (select e.*
     from eligible e, params p
     where e.title_missing = true and e.shuffle < p.threshold
     order by e.shuffle
     limit $1)
  ) q
  limit greatest($1 - (select n from title_present_count), 0)
),
featured as (
  select wad_id, meta_json, title_missing, shuffle from title_present
  union all
  select wad_id, meta_json, title_missing, shuffle from title_missing
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
group by f.title_missing, f.shuffle, f.wad_id, f.meta_json
order by f.title_missing asc, f.shuffle;
