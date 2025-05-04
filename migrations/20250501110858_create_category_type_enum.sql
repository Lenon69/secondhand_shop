-- Add migration script here
-- migrations/YYYYMMDDHHMMSS_create_category_type_enum.sql
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
);
