-- Dodanie kolumny user_id do produktów
ALTER TABLE products
ADD COLUMN user_id UUID;

-- Ustanowienie relacji klucza obcego, aby połączyć produkty z użytkownikami
ALTER TABLE products
ADD CONSTRAINT fk_products_user
FOREIGN KEY (user_id)
REFERENCES users(id);

-- Utworzenie wszystkich potrzebnych indeksów dla wydajności
CREATE INDEX IF NOT EXISTS idx_products_user_id ON products(user_id);
CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);
CREATE INDEX IF NOT EXISTS idx_products_category ON products(category);
CREATE INDEX IF NOT EXISTS idx_products_name ON products(name);
CREATE INDEX IF NOT EXISTS idx_products_price ON products(price);
