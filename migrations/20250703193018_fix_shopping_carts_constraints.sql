-- Krok 1: Usunięcie problematycznego ograniczenia NOT NULL z kolumny user_id.
-- To pozwoli na tworzenie koszyków dla gości, którzy nie mają user_id.
ALTER TABLE shopping_carts
ALTER COLUMN user_id DROP NOT NULL;

-- Krok 2 (Opcjonalny, ale BARDZO ZALECANY): Dodanie ograniczenia CHECK dla spójności danych.
-- To ograniczenie gwarantuje, że każdy koszyk jest przypisany albo do zalogowanego użytkownika (ma user_id),
-- albo do gościa (ma guest_session_id). Zapobiega to tworzeniu "osieroconych" koszyków.
-- UWAGA: Najpierw usuwamy stare ograniczenie, jeśli istnieje, aby uniknąć konfliktu.
ALTER TABLE shopping_carts
DROP CONSTRAINT IF EXISTS chk_cart_owner;

ALTER TABLE shopping_carts
ADD CONSTRAINT chk_cart_owner CHECK (
    -- Scenariusz 1: Koszyk należy do zalogowanego użytkownika.
    (user_id IS NOT NULL AND guest_session_id IS NULL) OR
    -- Scenariusz 2: Koszyk należy do gościa.
    (user_id IS NULL AND guest_session_id IS NOT NULL)
);
