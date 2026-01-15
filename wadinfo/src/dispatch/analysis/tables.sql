-- dispatch-analysis schema (owned by the dispatch-analysis microservice)

create table if not exists wad_dispatch_analysis (
  wad_id         uuid primary key references wads(wad_id) on delete cascade,
  dispatched_at  timestamptz not null default now()
);

create index if not exists idx_wad_dispatch_analysis_dispatched_at
  on wad_dispatch_analysis (dispatched_at);
