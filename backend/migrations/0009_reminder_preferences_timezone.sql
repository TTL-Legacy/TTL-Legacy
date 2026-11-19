-- Issue #1752: reminders fire at a local wall-clock hour, so preferences need
-- the IANA timezone whose offset is re-evaluated at every occurrence.
ALTER TABLE reminder_preferences ADD COLUMN timezone TEXT NOT NULL DEFAULT 'UTC';
