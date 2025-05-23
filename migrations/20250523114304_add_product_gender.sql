-- Plik migracji: YYYYMMDDHHMMSS_add_product_gender.sql

-- Tworzymy nowy typ ENUM dla płci produktu
CREATE TYPE product_gender AS ENUM ('Damskie', 'Meskie');

-- Dodajemy nową kolumnę 'gender' do tabeli 'products', na razie dopuszczając NULL
ALTER TABLE products
ADD COLUMN gender product_gender;

-- WAŻNY KROK: Aktualizacja istniejących rekordów
-- Musisz zdecydować, jak przypisać płeć do już istniejących produktów.
-- Poniżej jest BARDZO UPROSZCZONY przykład, który wszystkim istniejącym produktom
-- przypisuje 'Damskie'. POWINIENEŚ DOSTOSOWAĆ TĘ LOGIKĘ DO SWOICH DANYCH!
-- Możesz np. bazować na istniejących kategoriach, nazwach, lub ręcznie przejrzeć dane.
-- Przykład:
-- UPDATE products SET gender = 'Damskie' WHERE category::text ILIKE '%sukienka%' OR category::text ILIKE '%spódnica%';
-- UPDATE products SET gender = 'Meskie' WHERE category::text ILIKE '%koszula meska%' OR category::text ILIKE '%spodnie meskie%';
-- Jeśli nie możesz automatycznie sklasyfikować wszystkich, ustaw domyślną, a potem popraw ręcznie.
-- PONIŻEJ TYLKO PRZYKŁAD - NIE URUCHAMIAJ BEZ ZASTANOWIENIA!
UPDATE products
SET gender = 'Damskie' -- Lub 'Meskie', w zależności od tego, co jest bardziej prawdopodobne jako domyślne
WHERE gender IS NULL; -- Upewnij się, że aktualizujesz tylko te, które nie mają jeszcze wartości

-- Po zaktualizowaniu wszystkich istniejących wierszy,
-- zmieniamy kolumnę 'gender', aby wymagała wartości (NOT NULL)
ALTER TABLE products
ALTER COLUMN gender SET NOT NULL;

-- Możesz też dodać indeks na nowej kolumnie, jeśli będziesz często po niej filtrować
-- CREATE INDEX IF NOT EXISTS idx_products_gender ON products (gender);
