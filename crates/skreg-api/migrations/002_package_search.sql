CREATE TABLE package_search (
    package_id    UUID PRIMARY KEY REFERENCES packages(id),
    search_vector TSVECTOR NOT NULL
);

CREATE INDEX package_search_gin_idx ON package_search USING GIN (search_vector);

-- Keep search_vector in sync when packages table changes.
CREATE OR REPLACE FUNCTION update_package_search_vector()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    INSERT INTO package_search (package_id, search_vector)
    VALUES (
        NEW.id,
        to_tsvector('english', COALESCE(NEW.name, '') || ' ' || COALESCE(NEW.description, ''))
    )
    ON CONFLICT (package_id) DO UPDATE
        SET search_vector = to_tsvector(
            'english',
            COALESCE(NEW.name, '') || ' ' || COALESCE(NEW.description, '')
        );
    RETURN NEW;
END;
$$;

CREATE TRIGGER packages_search_sync
AFTER INSERT OR UPDATE ON packages
FOR EACH ROW EXECUTE FUNCTION update_package_search_vector();
