-- Studio heartbeat so agents can see the connector is actually attached.
create table if not exists studio_status (
  id           text primary key default 'global',
  recording    boolean not null default false,
  inspecting   boolean not null default false,
  source       text not null default 'idle',
  attached_at  timestamptz not null default now()
);

insert into studio_status (id) values ('global')
  on conflict (id) do nothing;
