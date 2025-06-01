-- Add migration script here
-- migrations/YYYYMMDDHHMMSS_add_full_shipping_details_to_orders.sql

ALTER TABLE orders
ADD COLUMN shipping_first_name TEXT,
ADD COLUMN shipping_last_name TEXT,
ADD COLUMN shipping_phone VARCHAR(30);

-- Po dodaniu kolumn, jeśli te dane są zawsze wymagane dla zamówienia,
-- możesz chcieć ustawić je jako NOT NULL.
-- Jeśli dodajesz je do istniejącej tabeli z danymi, musisz najpierw
-- uzupełnić te kolumny dla istniejących wierszy lub pozwolić na NULL na razie.
-- Dla nowych zamówień, zakładając, że formularz checkout wymaga tych pól,
-- można je ustawić jako NOT NULL.

-- Zakładając, że dla nowych zamówień te pola będą zawsze dostarczane:
-- (Jeśli masz już dane w tabeli 'orders', najpierw zaktualizuj istniejące wiersze
-- wartościami domyślnymi lub usuń `SET NOT NULL` i dodaj je później)
-- Na potrzeby tego przykładu, zakładamy, że możesz je dodać jako NOT NULL
-- jeśli tabela jest pusta lub jeśli możesz zaktualizować stare rekordy.
-- Jeśli nie, usuń klauzule SET NOT NULL na razie.

UPDATE orders SET shipping_first_name = 'BRAK' WHERE shipping_first_name IS NULL; -- Przykładowa aktualizacja dla istniejących danych
UPDATE orders SET shipping_last_name = 'BRAK' WHERE shipping_last_name IS NULL;  -- Przykładowa aktualizacja
UPDATE orders SET shipping_phone = '000000000' WHERE shipping_phone IS NULL;    -- Przykładowa aktualizacja

ALTER TABLE orders
ALTER COLUMN shipping_first_name SET NOT NULL,
ALTER COLUMN shipping_last_name SET NOT NULL,
ALTER COLUMN shipping_phone SET NOT NULL;

-- Istniejące kolumny adresowe (`shipping_address_line1`, `shipping_city`, etc.)
-- prawdopodobnie już istnieją i mają odpowiednie ograniczenia (np. NOT NULL).
-- Kolumna `shipping_address_line2` powinna pozostać TEXT NULL, ponieważ jest opcjonalna.
