-- Indeksy dla tabeli 'products'
CREATE INDEX IF NOT EXISTS idx_products_gender ON products (gender);
CREATE INDEX IF NOT EXISTS idx_products_price ON products (price);
CREATE INDEX IF NOT EXISTS idx_products_created_at ON products (created_at);
CREATE INDEX IF NOT EXISTS idx_products_on_sale ON products (on_sale);

-- Indeksy dla tabeli 'orders'
CREATE INDEX IF NOT EXISTS idx_orders_status ON orders (status);
CREATE INDEX IF NOT EXISTS idx_orders_order_date ON orders (order_date);

-- Indeksy dla tabeli 'order_items'
-- Ten indeks jest kluczowy do sprawdzania, czy produkt jest częścią zamówienia
CREATE INDEX IF NOT EXISTS idx_order_items_product_id ON order_items (product_id);

-- Indeksy dla tabeli 'shopping_carts'
CREATE INDEX IF NOT EXISTS idx_shopping_carts_user_id ON shopping_carts (user_id);
CREATE INDEX IF NOT EXISTS idx_shopping_carts_guest_session_id ON shopping_carts (guest_session_id);

-- Indeks dla tabeli 'cart_items'
CREATE INDEX IF NOT EXISTS idx_cart_items_product_id ON cart_items (product_id);

-- Indeks dla tabeli 'users'
CREATE INDEX IF NOT EXISTS idx_users_email ON users (email);

-- Indeks dla tabeli 'password_resets'
CREATE INDEX IF NOT EXISTS idx_password_resets_user_id ON password_resets (user_id);
