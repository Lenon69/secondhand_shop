-- Add migration script here
-- migrations/YYYYMMDDHHMMSS_alter_orders_for_guest_checkout.sql

-- Krok 1: Uczynienie user_id NULLABLE
ALTER TABLE orders
ALTER COLUMN user_id DROP NOT NULL; -- Lub ALTER COLUMN user_id SET NULL; w zależności od dialektu SQL

-- Krok 2: Dodanie kolumn dla zamówień gości
ALTER TABLE orders
ADD COLUMN guest_email TEXT,
ADD COLUMN guest_session_id UUID UNIQUE; -- Opcjonalne, jeśli chcesz powiązać z sesją gościa

-- Krok 3: Dodanie ograniczenia CHECK, aby przynajmniej jedno z user_id lub guest_email było ustawione
-- (lub user_id i guest_session_id, jeśli go używasz)
ALTER TABLE orders
ADD CONSTRAINT check_order_owner
CHECK (user_id IS NOT NULL OR guest_email IS NOT NULL);

-- Opcjonalnie: Dodaj indeksy dla nowych kolumn, jeśli przewidujesz wyszukiwanie po nich
CREATE INDEX IF NOT EXISTS idx_orders_guest_email ON orders(guest_email);
CREATE INDEX IF NOT EXISTS idx_orders_guest_session_id ON orders(guest_session_id);
