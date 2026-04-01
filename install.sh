#!/usr/bin/env bash
set -e

if [ "$EUID" -eq 0 ]; then
  echo "Please DO NOT run this script as root."
  echo "It will ask for sudo password internally when copying the binary to /usr/local/bin."
  exit 1
fi

# Run build
./build.sh

echo "==> Installing rtop to /usr/local/bin..."
sudo cp target/release/rtop /usr/local/bin/rtop

echo "==> Setting SUID bit (allow to run as root without sudo)"
sudo chown root:root /usr/local/bin/rtop
sudo chmod u+s /usr/local/bin/rtop

echo "==> Installation complete!"
echo ""
echo "You can now run 'rtop web' from anywhere to start the telemetry web dashboard."
echo ""
