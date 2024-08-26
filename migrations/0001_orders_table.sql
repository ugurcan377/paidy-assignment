CREATE TABLE IF NOT EXISTS orders(
    id SERIAL PRIMARY KEY,
    table_no INT NOT NULL,
    item_name VARCHAR(255) NOT NULL,
    duration INT NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    deleted_at TIMESTAMP
);