-- migrations/YYYYMMDDHHMMSS_add_payment_method_to_orders.sql

-- Krok 1: Stwórz nowy typ ENUM w PostgreSQL dla metod płatności
CREATE TYPE payment_method_enum AS ENUM ('blik', 'transfer');

-- Krok 2: Dodaj nową kolumnę do tabeli 'orders' używając tego typu
ALTER TABLE orders
ADD COLUMN payment_method payment_method_enum;

-- Opcjonalnie: Jeśli chcesz, aby ta kolumna była NOT NULL dla nowych zamówień,
-- możesz dodać to ograniczenie. Dla istniejących zamówień, wartość będzie NULL.
-- Jeśli chcesz ustawić domyślną wartość dla istniejących NULLi przed dodaniem NOT NULL:
-- UPDATE orders SET payment_method = 'transfer' WHERE payment_method IS NULL;
-- ALTER TABLE orders ALTER COLUMN payment_method SET NOT NULL;
