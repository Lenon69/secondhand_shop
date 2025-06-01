-- Add migration script here
-- migrations/YYYYMMDDHHMMSS_add_timestamps_to_products.sql

ALTER TABLE products
ADD COLUMN created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
ADD COLUMN updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW();

-- Trigger do automatycznej aktualizacji `updated_at`
-- (Jeśli już masz zdefiniowaną funkcję trigger_set_timestamp(), możesz pominąć jej tworzenie)
CREATE OR REPLACE FUNCTION trigger_set_timestamp()
RETURNS TRIGGER AS $$
BEGIN
  NEW.updated_at = NOW();
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER set_products_updated_at
BEFORE UPDATE ON products
FOR EACH ROW
EXECUTE FUNCTION trigger_set_timestamp();
