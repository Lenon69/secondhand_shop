-- Add migration script here
CREATE TYPE product_condition AS ENUM (
    'New',
    'LikeNew',
    'VeryGood',
    'Good',
    'Fair'
);
