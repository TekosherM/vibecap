-- Vibecap evidence studio — unowned rows (auth off).
create table if not exists sessions (
  id          text primary key,
  name        text not null,
  status      text not null default 'active',
  notes       text not null default '',
  created_at  timestamptz not null default now()
);

create table if not exists captures (
  id           text primary key,
  session_id   text not null references sessions(id) on delete cascade,
  kind         text not null,
  label        text not null default '',
  mime         text not null default 'image/jpeg',
  data_url     text,
  duration_ms  integer,
  created_at   timestamptz not null default now()
);
create index if not exists captures_session_idx on captures (session_id, created_at desc);

create table if not exists evidence (
  id          text primary key,
  session_id  text not null references sessions(id) on delete cascade,
  source      text not null,
  kind        text not null,
  title       text not null,
  body        text not null default '',
  capture_id  text,
  created_at  timestamptz not null default now()
);
create index if not exists evidence_session_idx on evidence (session_id, created_at desc);

create table if not exists packs (
  id          text primary key,
  session_id  text not null references sessions(id) on delete cascade,
  title       text not null,
  summary     text not null default '',
  payload     text not null,
  created_at  timestamptz not null default now()
);
create index if not exists packs_session_idx on packs (session_id, created_at desc);

create table if not exists inbox (
  id             text primary key,
  session_id     text,
  question       text not null,
  options        text not null default '[]',
  priority       text not null default 'normal',
  agent_label    text not null default 'agent',
  preferred      text not null default 'any',
  context        text not null default '',
  status         text not null default 'pending',
  answer_text    text,
  answer_choice  text,
  media_id       text,
  created_at     timestamptz not null default now()
);
create index if not exists inbox_status_idx on inbox (status, created_at desc);

create table if not exists logs (
  id          serial primary key,
  session_id  text,
  stream      text not null,
  level       text not null default 'info',
  message     text not null,
  meta        text not null default '',
  created_at  timestamptz not null default now()
);
create index if not exists logs_session_idx on logs (session_id, created_at desc);

create table if not exists commands (
  id          text primary key,
  tool        text not null,
  args        text not null default '{}',
  status      text not null default 'pending',
  result      text,
  created_at  timestamptz not null default now()
);
create index if not exists commands_status_idx on commands (status, created_at);

create table if not exists catalog_items (
  id           serial primary key,
  sku          text not null unique,
  name         text not null,
  price_cents  integer not null,
  stock        integer not null default 0
);

create table if not exists budget (
  id             text primary key default 'global',
  max_frames     integer not null default 80,
  max_mb         integer not null default 40,
  max_minutes    integer not null default 15,
  analysis_tier  text not null default 'standard',
  frames_used    integer not null default 0,
  mb_used        numeric not null default 0,
  minutes_used   numeric not null default 0
);

insert into budget (id) values ('global')
  on conflict (id) do nothing;

insert into catalog_items (sku, name, price_cents, stock) values
  ('LM-12', 'Linen tote', 1200, 18),
  ('LM-18', 'Safelight mug', 1800, 7),
  ('LM-15', 'Graphite notebook', 1500, 0)
  on conflict (sku) do nothing;
