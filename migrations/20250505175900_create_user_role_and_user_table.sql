-- Add migration script here
CREATE TYPE user_role AS ENUM (
    'admin',
    'customer'
);
