select
  w.wad_id,
  w.meta_json,
  (
    wm.map_json || jsonb_build_object(
      'images',
      coalesce(
        jsonb_agg(
          jsonb_build_object(
            'id', wmi.id,
            'url', wmi.url,
            'type', wmi.type
          )
          order by coalesce(wmi.type, ''), wmi.url, wmi.id
        ) filter (where wmi.id is not null),
        '[]'::jsonb
      )
    )
  ) as map_json
from wad_maps wm
join wads w on w.wad_id = wm.wad_id
left join wad_map_images wmi
  on wmi.wad_id = wm.wad_id
 and wmi.map_name = wm.map_name
where wm.wad_id = $1::uuid
  and wm.map_name = $2::text
group by w.wad_id, w.meta_json, wm.map_json
limit 1;
