-- Add migration script here
-- Plik: migrations/YYYYMMDDHHMMSS_update_shopping_carts_for_guest_users.sql

-- Krok 1: Uczyń kolumnę user_id nullable, jeśli wcześniej była NOT NULL.
-- Jeśli 'user_id' była już nullable, ta komenda może zwrócić błąd lub nic nie zrobić,
-- w zależności od systemu bazy danych. Dla PostgreSQL, jeśli jest już nullable, nie zrobi nic.
-- Jeśli constraint NOT NULL był nazwany, być może trzeba go usunąć po nazwie.
ALTER TABLE shopping_carts
ALTER COLUMN user_id DROP NOT NULL;

-- Krok 2: Dodaj nową kolumnę guest_session_id
ALTER TABLE shopping_carts
ADD COLUMN guest_session_id UUID;

-- Krok 3: Dodaj constraint UNIQUE dla guest_session_id
ALTER TABLE shopping_carts
ADD CONSTRAINT uq_shopping_carts_guest_session_id UNIQUE (guest_session_id);

-- Krok 4: Upewnij się, że constraint UNIQUE dla user_id nadal istnieje (jeśli był).
-- Jeśli constraint UNIQUE na user_id (np. shopping_carts_user_id_key) już istnieje i akceptuje wartości NULL
-- (co jest standardem w PostgreSQL - wiele NULLi jest dozwolonych w kolumnie UNIQUE),
-- to nie musisz go dodawać ponownie. Jeśli go nie ma lub został usunięty, dodaj go:
-- (Odkomentuj poniższe, jeśli jest potrzebne. Sprawdź aktualną definicję tabeli)
-- ALTER TABLE shopping_carts
-- ADD CONSTRAINT uq_shopping_carts_user_id UNIQUE (user_id);

-- Krok 5: Dodaj constraint CHK_CART_OWNER
ALTER TABLE shopping_carts
ADD CONSTRAINT chk_cart_owner CHECK (
    (user_id IS NOT NULL AND guest_session_id IS NULL) OR -- Koszyk zalogowanego użytkownika
    (user_id IS NULL AND guest_session_id IS NOT NULL) OR -- Koszyk gościa
    (user_id IS NULL AND guest_session_id IS NULL)       -- Pusty koszyk, jeszcze nieprzypisany (stan przejściowy, np. tuż po utworzeniu rekordu)
);

-- Krok 6 (Opcjonalnie, jeśli nie masz): Upewnij się, że cart_items ma ON DELETE CASCADE
-- Jeśli tabela cart_items już istnieje i nie ma ON DELETE CASCADE dla shopping_carts.id:
-- Najpierw usuń istniejący foreign key (jeśli znasz jego nazwę):
-- ALTER TABLE cart_items DROP CONSTRAINT nazwa_istniejacego_fk_cart_id;
-- Następnie dodaj nowy z ON DELETE CASCADE:
-- ALTER TABLE cart_items
-- ADD CONSTRAINT fk_cart_items_cart_id
-- FOREIGN KEY (cart_id) REFERENCES shopping_carts(id) ON DELETE CASCADE;
-- Jeśli tworzysz tabelę cart_items od nowa w tej lub innej migracji, uwzględnij to tam.
