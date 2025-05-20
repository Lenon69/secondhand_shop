-- Add migration script here
-- Propozycja modyfikacji/utworzenia tabeli shopping_carts
CREATE TABLE IF NOT EXISTS shopping_carts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID REFERENCES users(id) ON DELETE SET NULL, 
    guest_session_id UUID UNIQUE,                       
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_cart_owner CHECK (
        (user_id IS NOT NULL AND guest_session_id IS NULL) OR
        (user_id IS NULL AND guest_session_id IS NOT NULL) OR
        (user_id IS NULL AND guest_session_id IS NULL) -- Na krótko, zanim zostanie powiązany z gościem lub userem
    )
);
