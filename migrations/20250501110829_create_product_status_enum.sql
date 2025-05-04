-- Add migration script here
CREATE TYPE product_status AS ENUM (
    'Available',
    'Reserved',
    'Sold'
);
