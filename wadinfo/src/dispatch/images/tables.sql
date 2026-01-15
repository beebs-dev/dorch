-- dispatch-images schema (owned by the dispatch-images microservice)

create table if not exists wad_dispatch_images (
  wad_id         uuid primary key references wads(wad_id) on delete cascade,
  dispatched_at  timestamptz not null default now()
);

create index if not exists idx_wad_dispatch_images_dispatched_at
  on wad_dispatch_images (dispatched_at);
