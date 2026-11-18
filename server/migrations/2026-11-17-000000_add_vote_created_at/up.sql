ALTER TABLE votes ADD COLUMN created_at timestamptz;
UPDATE votes v SET created_at = s.created_at FROM scenarios s WHERE v.scenario_id = s.id;
ALTER TABLE votes ALTER COLUMN created_at SET NOT NULL;
