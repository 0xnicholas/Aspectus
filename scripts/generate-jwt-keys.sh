#!/bin/bash
# Generate JWT RSA key pair for Aspectus
# Usage: ./scripts/generate-jwt-keys.sh
# Output: jwt_private.pem + jwt_public.pem

set -e

echo "Generating RSA 2048-bit key pair for Aspectus JWT signing..."
openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -out jwt_private.pem 2>/dev/null
openssl rsa -pubout -in jwt_private.pem -out jwt_public.pem 2>/dev/null

echo ""
echo "✅ Keys generated:"
echo "   Private: jwt_private.pem"
echo "   Public:  jwt_public.pem"
echo ""
echo "Add to your .env:"
echo "   JWT_PRIVATE_KEY_PEM=$(cat jwt_private.pem | tr '\n' ' ')"
echo "   JWT_PUBLIC_KEY_PEM=$(cat jwt_public.pem | tr '\n' ' ')"
echo ""
echo "Or set as file paths:"
echo "   JWT_PRIVATE_KEY_PEM=/path/to/jwt_private.pem"
echo "   JWT_PUBLIC_KEY_PEM=/path/to/jwt_public.pem"
