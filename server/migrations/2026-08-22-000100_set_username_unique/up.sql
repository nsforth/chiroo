UPDATE users SET username = CASE
    WHEN email IS NOT NULL AND email <> '' THEN email
    ELSE 'user_' || id::text
END WHERE username IS NULL;

ALTER TABLE users ALTER COLUMN username SET NOT NULL;

CREATE UNIQUE INDEX users_username_key ON users (username);
