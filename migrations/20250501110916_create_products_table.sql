-- migrations/YYYYMMDDHHMMSS_create_products_table.sql
CREATE TABLE products (
    id UUID PRIMARY KEY NOT NULL,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    price BIGINT NOT NULL, -- Używamy 'price'
    condition product_condition NOT NULL, -- Używamy naszego typu ENUM
    category category_type NOT NULL, -- Używamy naszego typu ENUM
    status product_status NOT NULL, -- Używamy naszego typu ENUM
    images TEXT[] -- Tablica tekstów dla URL-i obrazków (PostgreSQL)
);
