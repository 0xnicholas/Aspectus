-- Add PKCE code_challenge storage to authorization_codes.
-- This column is required for OAuth2 /authorize endpoints that send
-- a code_challenge (RFC 7636). NULL means PKCE was not used.
ALTER TABLE authorization_codes
    ADD COLUMN IF NOT EXISTS code_challenge varchar(255);
