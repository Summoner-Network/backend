CREATE TABLE IF NOT EXISTS chunks (
    slot BIGINT NOT NULL,  -- Weak ordering of the chunk
    digest BYTEA NOT NULL, -- Hash of the chunk content
    series BYTEA NOT NULL, -- Vector of transaction hashes
    content BYTEA NOT NULL  -- Transactions in the chunk
);

CREATE TABLE IF NOT EXISTS blocks (
    rank BIGSERIAL NOT NULL,  -- Well ordering of the block
    digest BYTEA NOT NULL,    -- Hash of the block content
    series BYTEA NOT NULL,    -- Vector of contract hashes
    content BYTEA NOT NULL     -- Contractions in the block
);

CREATE TABLE IF NOT EXISTS settles (
    rank BIGINT NOT NULL,  -- Well ordering of the block
    reads BYTEA NOT NULL,
    writes BYTEA NOT NULL
);

-- contracts table
CREATE TABLE IF NOT EXISTS contracts (
    is_contract BOOLEAN NOT NULL,
    in_contract BYTEA NOT NULL,
    at_address BYTEA NOT NULL,
    data BYTEA NOT NULL,
    version BIGINT NOT NULL DEFAULT 0,
    CONSTRAINT unique_in_contract_at_address UNIQUE (in_contract, at_address)
);

CREATE INDEX IF NOT EXISTS idx_is_contract
    ON contracts (is_contract);

CREATE INDEX IF NOT EXISTS idx_in_contract_at_address
    ON contracts (in_contract, at_address);