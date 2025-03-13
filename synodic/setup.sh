#!/bin/bash

# Detect package manager (prefer yarn if available)
if command -v yarn &> /dev/null; then
  echo "Using yarn as package manager"
  PKG_MGR="yarn"
  INSTALL_CMD="yarn install"
  DEV_CMD="yarn dev"
elif command -v npm &> /dev/null; then
  echo "Using npm as package manager"
  PKG_MGR="npm"
  INSTALL_CMD="npm install"
  DEV_CMD="npm run dev"
else
  echo "Error: Neither yarn nor npm found. Please install one of them."
  exit 1
fi

# Install dependencies
echo "Installing dependencies..."
$INSTALL_CMD

# Success message
echo "====================================="
echo "Synodic Map setup complete!"
echo "To start the development server, run:"
echo "$DEV_CMD"
echo "=====================================" 