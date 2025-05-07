-- Add migration script here
-- Tworzenie typu ENUM dla statusu zamówienia (bez zmian)
CREATE TYPE order_status_enum AS ENUM (
    'pending',
    'processing',
    'shipped',
    'delivered',
    'cancelled'
);

-- Tworzenie tabeli orders (zamówienia) - bez pól bilingowych
CREATE TABLE orders (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    order_date TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    status order_status_enum NOT NULL,
    total_price BIGINT NOT NULL CHECK (total_price >= 0),

    shipping_address_line1 VARCHAR(255) NOT NULL,
    shipping_address_line2 VARCHAR(255),
    shipping_city VARCHAR(100) NOT NULL,
    shipping_postal_code VARCHAR(20) NOT NULL,
    shipping_country VARCHAR(100) NOT NULL,

    -- Pola bilingowe USUNIĘTE

    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Indeksy dla orders (bez zmian)
CREATE INDEX idx_orders_user_id ON orders(user_id);
CREATE INDEX idx_orders_status ON orders(status);

-- Trigger do aktualizacji updated_at w tabeli orders (bez zmian, zakładając, że funkcja już istnieje lub jest tu tworzona)
CREATE OR REPLACE FUNCTION update_modified_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ language 'plpgsql';

CREATE TRIGGER update_orders_updated_at
BEFORE UPDATE ON orders
FOR EACH ROW
EXECUTE FUNCTION update_modified_column();


-- Tworzenie tabeli order_items (pozycje zamówienia) - bez quantity
CREATE TABLE order_items (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    order_id UUID NOT NULL REFERENCES orders(id) ON DELETE CASCADE,
    product_id UUID NOT NULL REFERENCES products(id) ON DELETE RESTRICT,
    -- quantity INTEGER NOT NULL CHECK (quantity > 0), -- USUNIĘTE
    price_at_purchase BIGINT NOT NULL CHECK (price_at_purchase >= 0)
);

-- Indeksy dla order_items (bez zmian)
CREATE INDEX idx_order_items_order_id ON order_items(order_id);
CREATE INDEX idx_order_items_product_id ON order_items(product_id);

-- DODATKOWO: Dodajmy ograniczenie UNIQUE, aby ten sam produkt nie mógł być dodany dwa razy do tego samego zamówienia
ALTER TABLE order_items ADD CONSTRAINT unique_order_product UNIQUE (order_id, product_id);
