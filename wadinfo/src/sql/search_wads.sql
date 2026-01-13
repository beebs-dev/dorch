select
  count(*) over() as full_count,
  w.wad_id,
  w.meta_json,
  greatest(
    similarity(coalesce(w.title, ''), $1),
    similarity(coalesce(a.authors, ''), $1),
    similarity(coalesce(d.descriptions, ''), $1),
    case when w.sha1 = $1 then 1 else 0 end
  ) as rank
from wads w
left join lateral (
  select string_agg(author, ' ' order by ord) as authors
  from wad_authors
  where wad_id = w.wad_id
) a on true
left join lateral (
  select string_agg(description, ' ' order by ord) as descriptions
  from wad_descriptions
  where wad_id = w.wad_id
) d on true
where coalesce(w.title, '') % $1
   or coalesce(a.authors, '') % $1
   or coalesce(d.descriptions, '') % $1
   or w.sha1 = $1
order by rank desc, coalesce(w.title, '')
offset $2
limit $3;
