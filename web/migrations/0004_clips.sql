-- Persist WebM clips next to the JPEG poster so reload still has Download clip.
alter table captures add column if not exists clip_url text;
