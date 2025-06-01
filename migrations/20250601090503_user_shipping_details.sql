-- migrations/YYYYMMDDHHMMSS_create_user_shipping_details.sql

CREATE TABLE user_shipping_details (
    user_id UUID PRIMARY KEY NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    shipping_first_name TEXT,
    shipping_last_name TEXT,
    shipping_address_line1 TEXT,
    shipping_address_line2 TEXT,
    shipping_city TEXT,
    shipping_postal_code VARCHAR(20), -- Kody pocztowe mają zwykle ograniczoną długość
    shipping_country VARCHAR(100),
    shipping_phone VARCHAR(30),     -- Numery telefonów również
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Trigger do automatycznej aktualizacji `updated_at`
CREATE OR REPLACE FUNCTION trigger_set_timestamp()
RETURNS TRIGGER AS $$
BEGIN
  NEW.updated_at = NOW();
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER set_user_shipping_details_updated_at
BEFORE UPDATE ON user_shipping_details
FOR EACH ROW
EXECUTE FUNCTION trigger_set_timestamp();-- Add migration script here
