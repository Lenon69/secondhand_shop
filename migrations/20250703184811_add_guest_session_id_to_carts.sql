-- Dodanie kolumny do przechowywania ID sesji gościa w tabeli z koszykami
-- Zakładam, że tabela z koszykami nazywa się `carts`. Jeśli jest inaczej, zmień nazwę w poniższych poleceniach.
ALTER TABLE shopping_carts
ADD COLUMN guest_session_id UUID;

-- Stworzenie indeksu na nowej kolumnie, aby przyspieszyć wyszukiwanie koszyków gości
CREATE INDEX IF NOT EXISTS idx_carts_guest_session_id ON shopping_carts(guest_session_id);
