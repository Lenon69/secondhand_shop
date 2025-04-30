-- Add migration script here
-- Stworzenie typu ENUM dla kategorii
CREATE TYPE category_type AS ENUM (
    'Koszule',
    'Spodnie',
    'Sukienki',
    'Spodnice',
    'Swetry',
    'Bluzy',
    'KurtkiPlaszcze',
    'MarynarkiZakiety',
    'Obuwie',
    'Torebki',
    'Akcesoria',
    'Bielizna',
    'StrojeKapielowe',
    'Inne'
    -- Dodaj/usuń zgodnie z definicją enuma w Rust
);

-- Zmiana typu kolumny 'category' w tabeli 'products'
-- UWAGA: To zadziała tylko jeśli istniejące wartości w kolumnie 'category'
--        dokładnie pasują (z uwzględnieniem wielkości liter) do wartości w ENUM.
--        Jeśli masz inne wartości, konwersja USING może zawieść.
--        Bezpieczniejszą opcją może być dodanie nowej kolumny, migracja danych,
--        usunięcie starej i zmiana nazwy nowej, ale spróbujmy najpierw tak:
ALTER TABLE products
ALTER COLUMN category TYPE category_type
USING category::category_type; -- Próba rzutowania istniejących stringów na nowy typ enum
