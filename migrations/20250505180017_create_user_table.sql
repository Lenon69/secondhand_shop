-- Add migration script here
CREATE TABLE users (
    id UUID PRIMARY KEY NOT NULL DEFAULT gen_random_uuid(),
    email VARCHAR(255) UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    role user_role NOT NULL DEFAULT 'customer'
);
