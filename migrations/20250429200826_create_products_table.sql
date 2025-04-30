-- Add migration script here
CREATE TYPE product_condition AS ENUM (
    'New'
    'LikeNew'
    'VeryGood'
    'Good'
    'Fair'
);

CREATE TYPE product_status AS ENUM (
    'Available'
    'Reserved'
    'Sold'
);

CREATE TABLE products (
    id UUID PRIMARY KEY NOT NULL,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    price BIGINT NOT NULL,
    condition product_condition NOT NULL,
    category VARCHAR(100) NOT NULL,
    status product_status NOT NULL,
    images TEXT[] 
)
