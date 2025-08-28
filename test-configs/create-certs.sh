#!/bin/bash

# Create self-signed certificate for ODH Gateway testing
# This creates the gwapi-wildcard secret that the Gateway expects

set -e

DOMAIN=${1:-"apps-crc.testing"}
SECRET_NAME="gwapi-wildcard"
NAMESPACE="openshift-ingress"

echo "Creating self-signed certificate for domain: *.$DOMAIN"

# Create temporary directory for cert files
TEMP_DIR=$(mktemp -d)
cd "$TEMP_DIR"

# Generate private key
openssl genrsa -out tls.key 2048

# Generate certificate signing request and certificate
openssl req -new -x509 -key tls.key -out tls.crt -days 365 -subj "/CN=*.$DOMAIN"

# Create the TLS secret in openshift-ingress namespace
echo "Creating TLS secret $SECRET_NAME in namespace $NAMESPACE..."
oc create secret tls "$SECRET_NAME" --cert=tls.crt --key=tls.key -n "$NAMESPACE" --dry-run=client -o yaml | oc apply -f -

# Clean up
cd - > /dev/null
rm -rf "$TEMP_DIR"

echo "✅ TLS secret $SECRET_NAME created successfully"
echo "✅ Gateway should now be able to terminate TLS connections"
